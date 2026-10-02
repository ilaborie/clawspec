use headers::ContentType;
use mime::Mime;
use serde::de::DeserializeOwned;
use tracing::{debug, warn};
use utoipa::openapi::{ObjectBuilder, RefOr, Schema, Type};

use crate::client::ApiClientError;

pub(in crate::client) const JSON_SEQUENCE_MEDIA_TYPES: &str =
    "application/jsonl, application/x-ndjson or application/json-seq";
pub(in crate::client) const EVENT_STREAM_MEDIA_TYPE: &str = "text/event-stream";

const RECORD_SEPARATOR: char = '\u{1e}';
const BYTE_ORDER_MARK: char = '\u{feff}';
const UNTERMINATED_PREVIEW_CHARS: usize = 40;

/// A server-sent event whose `data` field holds a JSON value.
///
/// Returned by [`CallResult::as_sse`](crate::CallResult::as_sse).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SseEvent<T> {
    /// The event type, `None` when the `event` field is missing or empty.
    pub event: Option<String>,
    /// The `data` field (multiple `data` lines joined with a line feed) parsed as JSON.
    pub data: T,
    /// The `id` field set in this event block, if any.
    ///
    /// It is not carried over to the next events, unlike the last event ID of a browser.
    pub id: Option<String>,
    /// The `retry` field of this event in milliseconds, if valid.
    pub retry: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::client) enum SequentialKind {
    JsonLines,
    JsonSeq,
    EventStream,
}

pub(in crate::client) fn sequential_kind(content_type: &ContentType) -> Option<SequentialKind> {
    let mime = Mime::from(content_type.clone());
    match mime.essence_str() {
        "application/jsonl" | "application/x-ndjson" => Some(SequentialKind::JsonLines),
        "application/json-seq" => Some(SequentialKind::JsonSeq),
        EVENT_STREAM_MEDIA_TYPE => Some(SequentialKind::EventStream),
        _ => None,
    }
}

pub(in crate::client) const STREAM_EXAMPLE_ITEM_COUNT: usize = 3;

#[derive(Debug)]
pub(in crate::client) struct ParsedStream<T> {
    pub(in crate::client) items: Vec<T>,
    pub(in crate::client) example: Option<String>,
}

pub(in crate::client) fn parse_json_sequence<T>(
    kind: SequentialKind,
    body: &str,
) -> Result<ParsedStream<T>, ApiClientError>
where
    T: DeserializeOwned,
{
    let (records, separator) = match kind {
        SequentialKind::JsonLines => (json_lines_records(body), "\n"),
        SequentialKind::JsonSeq => (json_seq_records(body), ""),
        SequentialKind::EventStream => {
            return Err(ApiClientError::UnexpectedOutputType {
                expected: JSON_SEQUENCE_MEDIA_TYPES.to_string(),
                actual: EVENT_STREAM_MEDIA_TYPE.to_string(),
            });
        }
    };
    let items = records
        .iter()
        .enumerate()
        .map(|(index, record)| deserialize_item(&format!("[{index}]"), record.json))
        .collect::<Result<Vec<_>, _>>()?;
    let example = stream_example(records.iter().map(|record| record.raw), separator);
    Ok(ParsedStream { items, example })
}

pub(in crate::client) fn parse_sse<T>(
    body: &str,
) -> Result<ParsedStream<SseEvent<T>>, ApiClientError>
where
    T: DeserializeOwned,
{
    let raw_events = parse_event_stream(body);
    let example = stream_example(raw_events.iter().map(|raw| raw.raw), "");
    let items = raw_events
        .into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let data = deserialize_item(&format!("[{index}].data"), &raw.data)?;
            Ok(SseEvent {
                event: raw.event,
                data,
                id: raw.id,
                retry: raw.retry,
            })
        })
        .collect::<Result<Vec<_>, ApiClientError>>()?;
    Ok(ParsedStream { items, example })
}

fn stream_example<'a>(raw_items: impl Iterator<Item = &'a str>, separator: &str) -> Option<String> {
    let example =
        raw_items
            .take(STREAM_EXAMPLE_ITEM_COUNT)
            .fold(String::new(), |mut example, raw| {
                example.push_str(raw);
                example.push_str(separator);
                example
            });
    (!example.is_empty()).then_some(example)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceRecord<'a> {
    raw: &'a str,
    json: &'a str,
}

fn json_lines_records(body: &str) -> Vec<SequenceRecord<'_>> {
    body.lines()
        .map(|line| SequenceRecord {
            raw: line,
            json: line.trim(),
        })
        .filter(|record| !record.json.is_empty())
        .collect()
}

fn json_seq_records(body: &str) -> Vec<SequenceRecord<'_>> {
    let mut boundaries = body
        .match_indices(RECORD_SEPARATOR)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if boundaries.first() != Some(&0) {
        boundaries.insert(0, 0);
    }
    boundaries.push(body.len());
    boundaries
        .windows(2)
        .map(|window| {
            let raw = &body[window[0]..window[1]];
            let json = raw.strip_prefix(RECORD_SEPARATOR).unwrap_or(raw).trim();
            SequenceRecord { raw, json }
        })
        .filter(|record| !record.json.is_empty())
        .collect()
}

/// Builds the item schema describing one server-sent event whose `data` holds `data_schema` as JSON.
pub(in crate::client) fn sse_event_schema(data_schema: RefOr<Schema>) -> RefOr<Schema> {
    let string = || ObjectBuilder::new().schema_type(Type::String);
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property(
            "data",
            string()
                .content_media_type(mime::APPLICATION_JSON.as_ref())
                .content_schema(Some(data_schema)),
        )
        .required("data")
        .property("event", string())
        .property("id", string())
        .property(
            "retry",
            ObjectBuilder::new()
                .schema_type(Type::Integer)
                .minimum(Some(0)),
        )
        .into()
}

fn deserialize_item<T>(prefix: &str, item: &str) -> Result<T, ApiClientError>
where
    T: DeserializeOwned,
{
    let deserializer = &mut serde_json::Deserializer::from_str(item);
    serde_path_to_error::deserialize(deserializer).map_err(|err| {
        let inner_path = err.path().to_string();
        let path = match inner_path.as_str() {
            "." => prefix.to_string(),
            nested if nested.starts_with('[') => format!("{prefix}{nested}"),
            nested => format!("{prefix}.{nested}"),
        };
        ApiClientError::JsonError {
            path,
            error: err.into_inner(),
            body: item.to_string(),
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawSseEvent<'a> {
    raw: &'a str,
    event: Option<String>,
    data: String,
    id: Option<String>,
    retry: Option<u64>,
}

#[derive(Debug, Default)]
struct SseEventBuffer {
    event: Option<String>,
    data: Option<String>,
    id: Option<String>,
    retry: Option<u64>,
}

impl SseEventBuffer {
    fn is_empty(&self) -> bool {
        self.event.is_none() && self.data.is_none() && self.id.is_none() && self.retry.is_none()
    }

    fn process_line(&mut self, line: &str) {
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_string()).filter(|event| !event.is_empty()),
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                }
                None => self.data = Some(value.to_string()),
            },
            "id" if !value.contains('\0') => self.id = Some(value.to_string()),
            "retry" if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
                self.retry = value.parse().ok();
            }
            _ => {}
        }
    }

    fn dispatch<'a>(&mut self, raw: &'a str) -> Option<RawSseEvent<'a>> {
        let Self {
            event,
            data,
            id,
            retry,
        } = std::mem::take(self);
        let data = data.filter(|data| {
            if data.is_empty() {
                debug!("skipping server-sent event with empty data");
            }
            !data.is_empty()
        });
        data.map(|data| RawSseEvent {
            raw,
            event,
            data,
            id,
            retry,
        })
    }
}

fn parse_event_stream(body: &str) -> Vec<RawSseEvent<'_>> {
    let body = body.strip_prefix(BYTE_ORDER_MARK).unwrap_or(body);
    let mut events = Vec::new();
    let mut buffer = SseEventBuffer::default();
    let mut block_start = 0;
    let mut rest = body;
    while let Some(end) = rest.find(['\r', '\n']) {
        let line = &rest[..end];
        let terminator_len = if rest[end..].starts_with("\r\n") {
            2
        } else {
            1
        };
        rest = &rest[end + terminator_len..];
        if line.is_empty() {
            let block_end = body.len() - rest.len();
            events.extend(buffer.dispatch(&body[block_start..block_end]));
            block_start = block_end;
        } else {
            buffer.process_line(line);
        }
    }
    if !rest.is_empty() || !buffer.is_empty() {
        let unterminated = &body[block_start..];
        let preview = unterminated
            .chars()
            .take(UNTERMINATED_PREVIEW_CHARS)
            .collect::<String>();
        warn!(
            bytes = unterminated.len(),
            %preview,
            "discarding unterminated server-sent event at the end of the stream"
        );
    }
    events
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Item {
        id: u32,
        name: String,
    }

    fn content_type(value: &str) -> ContentType {
        ContentType::from(value.parse::<Mime>().expect("should be a valid mime"))
    }

    #[test]
    fn should_detect_sequential_kind() {
        let kinds = [
            "application/jsonl",
            "application/x-ndjson; charset=utf-8",
            "application/json-seq",
            "text/event-stream; charset=utf-8",
            "application/json",
        ]
        .map(|value| sequential_kind(&content_type(value)));

        assert_eq!(
            kinds,
            [
                Some(SequentialKind::JsonLines),
                Some(SequentialKind::JsonLines),
                Some(SequentialKind::JsonSeq),
                Some(SequentialKind::EventStream),
                None,
            ]
        );
    }

    #[test]
    fn should_parse_json_lines_skipping_blank_lines() {
        let body = "{\"id\":1,\"name\":\"a\"}\r\n\n   \n{\"id\":2,\"name\":\"b\"}";

        let items = parse_json_sequence::<Item>(SequentialKind::JsonLines, body)
            .expect("should parse JSON lines")
            .items;

        assert_debug_snapshot!(items, @r#"
        [
            Item {
                id: 1,
                name: "a",
            },
            Item {
                id: 2,
                name: "b",
            },
        ]
        "#);
    }

    #[test]
    fn should_parse_json_seq_with_record_separators_and_line_feeds() {
        let body = "\u{1e}{\"id\":1,\"name\":\"a\"}\n\u{1e}{\"id\":2,\n\"name\":\"b\"}\n\u{1e}\n";

        let items = parse_json_sequence::<Item>(SequentialKind::JsonSeq, body)
            .expect("should parse JSON text sequence")
            .items;

        assert_debug_snapshot!(items, @r#"
        [
            Item {
                id: 1,
                name: "a",
            },
            Item {
                id: 2,
                name: "b",
            },
        ]
        "#);
    }

    #[test]
    fn should_report_item_index_in_error_path() {
        let body = (0..3)
            .map(|id| format!("{{\"id\":{id},\"name\":\"n\"}}"))
            .chain(["{\"id\":3,\"name\":42}".to_string()])
            .collect::<Vec<_>>()
            .join("\n");

        let error = parse_json_sequence::<Item>(SequentialKind::JsonLines, &body)
            .expect_err("should fail on the fourth item");

        let ApiClientError::JsonError { path, body, .. } = error else {
            panic!("expected a JSON error, got {error:?}");
        };
        assert_eq!(path, "[3].name");
        assert_eq!(body, "{\"id\":3,\"name\":42}");
    }

    #[test]
    fn should_report_sse_data_path_in_error() {
        let body = "data: {\"id\":1,\"name\":\"a\"}\n\ndata: {\"id\":\"x\",\"name\":\"b\"}\n\n";

        let error = parse_sse::<Item>(body).expect_err("should fail on the second event");

        let ApiClientError::JsonError { path, .. } = error else {
            panic!("expected a JSON error, got {error:?}");
        };
        assert_eq!(path, "[1].data.id");
    }

    #[test]
    fn should_parse_sse_fields() {
        let body = "event: created\nid: 42\nretry: 3000\ndata: {\"id\":1,\"name\":\"a\"}\n\n";

        let events = parse_sse::<Item>(body)
            .expect("should parse event stream")
            .items;

        assert_debug_snapshot!(events, @r#"
        [
            SseEvent {
                event: Some(
                    "created",
                ),
                data: Item {
                    id: 1,
                    name: "a",
                },
                id: Some(
                    "42",
                ),
                retry: Some(
                    3000,
                ),
            },
        ]
        "#);
    }

    #[test]
    fn should_strip_bom_and_handle_all_line_endings() {
        let body = "\u{feff}data: 1\r\n\r\ndata: 2\r\rdata: 3\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events.iter().map(|event| event.data.as_str()).collect::<Vec<_>>(), @r#"
        [
            "1",
            "2",
            "3",
        ]
        "#);
    }

    #[test]
    fn should_ignore_comments() {
        let body = ": keep-alive\n\n: comment\ndata: 1\n: another\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: ": comment\ndata: 1\n: another\n\n",
                event: None,
                data: "1",
                id: None,
                retry: None,
            },
        ]
        "#);
    }

    #[test]
    fn should_strip_only_one_leading_space() {
        let body = "data:no-space\ndata:  two-spaces\nevent:  spaced\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: "data:no-space\ndata:  two-spaces\nevent:  spaced\n\n",
                event: Some(
                    " spaced",
                ),
                data: "no-space\n two-spaces",
                id: None,
                retry: None,
            },
        ]
        "#);
    }

    #[test]
    fn should_treat_line_without_colon_as_field_with_empty_value() {
        let body = "data\ndata\n\nevent\ndata: x\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: "data\ndata\n\n",
                event: None,
                data: "\n",
                id: None,
                retry: None,
            },
            RawSseEvent {
                raw: "event\ndata: x\n\n",
                event: None,
                data: "x",
                id: None,
                retry: None,
            },
        ]
        "#);
    }

    #[test]
    fn should_join_multiple_data_lines() {
        let body = "data: {\ndata: \"id\": 1,\ndata: \"name\": \"a\"\ndata: }\n\n";

        let events = parse_sse::<Item>(body)
            .expect("should parse multi-line data")
            .items;

        assert_debug_snapshot!(events[0].data, @r#"
        Item {
            id: 1,
            name: "a",
        }
        "#);
    }

    #[test]
    fn should_ignore_id_with_nul_and_non_numeric_retry() {
        let body =
            "id: a\0b\nretry: 12a\nretry: -5\nretry:\ndata: 1\n\nid: ok\nretry: 0010\ndata: 2\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: "id: a\0b\nretry: 12a\nretry: -5\nretry:\ndata: 1\n\n",
                event: None,
                data: "1",
                id: None,
                retry: None,
            },
            RawSseEvent {
                raw: "id: ok\nretry: 0010\ndata: 2\n\n",
                event: None,
                data: "2",
                id: Some(
                    "ok",
                ),
                retry: Some(
                    10,
                ),
            },
        ]
        "#);
    }

    #[test]
    fn should_not_dispatch_block_without_data() {
        let body = "event: ping\nid: 1\n\ndata: 2\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: "data: 2\n\n",
                event: None,
                data: "2",
                id: None,
                retry: None,
            },
        ]
        "#);
    }

    #[test]
    fn should_skip_events_with_empty_data() {
        let body = "data: 1\n\nevent: ping\ndata:\n\ndata: \n\ndata: 2\n\n";

        let events = parse_event_stream(body);

        assert_debug_snapshot!(events, @r#"
        [
            RawSseEvent {
                raw: "data: 1\n\n",
                event: None,
                data: "1",
                id: None,
                retry: None,
            },
            RawSseEvent {
                raw: "data: 2\n\n",
                event: None,
                data: "2",
                id: None,
                retry: None,
            },
        ]
        "#);
    }

    #[test]
    fn should_not_count_heartbeats_in_example() {
        let body = "data: {\"id\":1,\"name\":\"a\"}\n\nevent: ping\ndata:\n\ndata: {\"id\":2,\"name\":\"b\"}\n\ndata: {\"id\":3,\"name\":\"c\"}\n\n";

        let parsed = parse_sse::<Item>(body).expect("should skip the heartbeat");

        assert_eq!(parsed.items.len(), 3);
        assert_debug_snapshot!(parsed.example, @r#"
        Some(
            "data: {\"id\":1,\"name\":\"a\"}\n\ndata: {\"id\":2,\"name\":\"b\"}\n\ndata: {\"id\":3,\"name\":\"c\"}\n\n",
        )
        "#);
    }

    #[test]
    fn should_report_truncated_json_seq_record() {
        let body = "\u{1e}{\"id\":0,\"name\":\"a\"}\n\u{1e}{\"id\":1";

        let error = parse_json_sequence::<Item>(SequentialKind::JsonSeq, body)
            .expect_err("should fail on the truncated record");

        let ApiClientError::JsonError { path, body, .. } = error else {
            panic!("expected a JSON error, got {error:?}");
        };
        assert_eq!(path, "[1].?");
        assert_eq!(body, "{\"id\":1");
    }

    #[test]
    fn should_discard_unterminated_last_event() {
        let body = "data: 1\n\ndata: 2\n";

        let events = parse_event_stream(body);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "1");
    }

    #[test]
    fn should_record_first_json_lines_as_example() {
        let body = (1..=5)
            .map(|id| format!("{{\"id\":{id},\"name\":\"n\"}} "))
            .collect::<Vec<_>>()
            .join("\r\n\r\n");

        let parsed = parse_json_sequence::<Item>(SequentialKind::JsonLines, &body)
            .expect("should parse JSON lines");

        assert_eq!(parsed.items.len(), 5);
        assert_debug_snapshot!(parsed.example, @r#"
        Some(
            "{\"id\":1,\"name\":\"n\"} \n{\"id\":2,\"name\":\"n\"} \n{\"id\":3,\"name\":\"n\"} \n",
        )
        "#);
    }

    #[test]
    fn should_keep_json_seq_records_as_on_the_wire() {
        let body = "{\"id\":0,\"name\":\"a\"}\n\u{1e}{\"id\":1,\n\"name\":\"b\"}\r\n\u{1e}\n\u{1e} {\"id\":2,\"name\":\"c\"}\u{1e}{\"id\":3,\"name\":\"d\"}\n";

        let records = json_seq_records(body)
            .into_iter()
            .map(|record| record.raw)
            .collect::<Vec<_>>();
        let parsed = parse_json_sequence::<Item>(SequentialKind::JsonSeq, body)
            .expect("should parse JSON text sequence");

        assert_debug_snapshot!(records, @r#"
        [
            "{\"id\":0,\"name\":\"a\"}\n",
            "\u{1e}{\"id\":1,\n\"name\":\"b\"}\r\n",
            "\u{1e} {\"id\":2,\"name\":\"c\"}",
            "\u{1e}{\"id\":3,\"name\":\"d\"}\n",
        ]
        "#);
        assert_debug_snapshot!(parsed.example, @r#"
        Some(
            "{\"id\":0,\"name\":\"a\"}\n\u{1e}{\"id\":1,\n\"name\":\"b\"}\r\n\u{1e} {\"id\":2,\"name\":\"c\"}",
        )
        "#);
    }

    #[test]
    fn should_keep_raw_sse_blocks() {
        let body = ": keep-alive\r\n\r\nevent: a\r\n: inside\r\ndata: {\r\ndata: \"id\": 1, \"name\": \"a\"}\r\n\r\ndata: 2\n\n";

        let raw_blocks = parse_event_stream(body)
            .into_iter()
            .map(|event| event.raw)
            .collect::<Vec<_>>();

        assert_debug_snapshot!(raw_blocks, @r#"
        [
            "event: a\r\n: inside\r\ndata: {\r\ndata: \"id\": 1, \"name\": \"a\"}\r\n\r\n",
            "data: 2\n\n",
        ]
        "#);
    }

    #[test]
    fn should_record_first_sse_events_as_example() {
        let body = (1..=5)
            .map(|id| format!("id: {id}\ndata: {{\"id\":{id},\"name\":\"n\"}}\n\n"))
            .collect::<String>();

        let parsed = parse_sse::<Item>(&body).expect("should parse event stream");

        assert_eq!(parsed.items.len(), 5);
        assert_debug_snapshot!(parsed.example, @r#"
        Some(
            "id: 1\ndata: {\"id\":1,\"name\":\"n\"}\n\nid: 2\ndata: {\"id\":2,\"name\":\"n\"}\n\nid: 3\ndata: {\"id\":3,\"name\":\"n\"}\n\n",
        )
        "#);
    }

    #[test]
    fn should_not_record_example_for_empty_stream() {
        let sequence = parse_json_sequence::<Item>(SequentialKind::JsonLines, "\n\n")
            .expect("should parse empty JSON lines");
        let events = parse_sse::<Item>(": only a comment\n\n").expect("should parse empty stream");

        assert_eq!(sequence.example, None);
        assert_eq!(events.example, None);
    }

    #[test]
    fn should_build_sse_event_schema() {
        let schema = sse_event_schema(RefOr::Ref(utoipa::openapi::Ref::from_schema_name("Item")));

        insta::assert_snapshot!(
            serde_saphyr::to_string(&schema).expect("should serialize to YAML"),
            @r##"
        type: object
        required:
        - data
        properties:
          data:
            type: string
            contentMediaType: application/json
            contentSchema:
              $ref: "#/components/schemas/Item"
          event:
            type: string
          id:
            type: string
          retry:
            type: integer
            minimum: 0
        "##
        );
    }
}
