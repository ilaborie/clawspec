We need do introduce a trait to represent the ability to transform the `CallResult` into a generic value.

Something like:

```rust
trait ResultCollector {
  type Output;
  type Err: std::error::Error;

  fn collect(&mut CallResult) -> Result<Self::Output, Self::Err);
}
```

So the `as_json`, `as_text`, `as_bytes`, ... and the `as_optional_json`, `as_result_json` need can use the logic of the trait.

For example the `JsonCollector` is implementing `ResultCollector` for an `Output` that is `DeserializeOwned + ToSchema + 'static`.

The `ApiClientError` should have a variant to wrap the collector error.

Follow DRY, KISS, YAGNI to implements that

Ultrathink, and build a plan.md

Be careful to correctly document and test the feature.

We probably want to write all this code in @lib/clawspec-core/src/client/results.rs module
