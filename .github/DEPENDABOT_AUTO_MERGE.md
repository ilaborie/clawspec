# Dependabot Auto-merge Configuration

This document explains the automated dependency update system configured for this repository.

## Overview

The repository uses Dependabot with intelligent auto-merge capabilities to automatically handle routine dependency updates while requiring manual review for potentially breaking changes.

## Auto-merge Criteria

### ✅ Automatically Merged

The following types of updates are automatically approved and merged when all CI checks pass:

#### Rust Dependencies (`cargo`)
- **Patch updates** (e.g., 1.2.3 → 1.2.4)
  - Direct production dependencies
  - Direct development dependencies  
  - Indirect dependencies
- **Security updates** (regardless of version type)

#### GitHub Actions
- **Patch updates** (e.g., v1.2.3 → v1.2.4)
- **Minor updates** (e.g., v1.2 → v1.3)

### ⚠️ Manual Review Required

The following updates require manual review and approval:

#### Rust Dependencies
- **Major version updates** (e.g., 1.x → 2.x)
  - May contain breaking changes
  - Require careful API compatibility review
- **Minor version updates** (e.g., 1.2 → 1.3)
  - May introduce new features that need validation
  - Could affect behavior or performance

#### GitHub Actions
- **Major version updates** (e.g., v1 → v2)
  - Significant changes in behavior or requirements

## How It Works

### 1. Dependabot Configuration (`.github/dependabot.yml`)

```yaml
# Rust dependencies with auto-merge enabled for patch updates
- package-ecosystem: "cargo"
  allow:
    - dependency-type: "direct:production"
      update-type: "version-update:semver-patch"
    - dependency-type: "direct:development"
      update-type: "version-update:semver-patch"
    - dependency-type: "indirect"
      update-type: "version-update:semver-patch"

# GitHub Actions with auto-merge for patch and minor updates
- package-ecosystem: "github-actions"
  allow:
    - dependency-type: "direct:production"
      update-type: "version-update:semver-patch"
    - dependency-type: "direct:production"
      update-type: "version-update:semver-minor"
```

### 2. Auto-merge Workflow (`.github/workflows/dependabot-auto-merge.yml`)

The workflow automatically:

1. **Detects Dependabot PRs** - Only runs for PRs created by `dependabot[bot]`

2. **Analyzes Update Type** - Parses PR title to determine if it's a patch, minor, or major update

3. **Checks Security Status** - Identifies security updates by scanning PR title and description

4. **Waits for CI** - Monitors all required status checks:
   - ✅ Check Code Quality
   - 🔒 Security Audit (allowed to fail)
   - 🦀 Minimum Supported Rust Version
   - 📦 Minimal Dependency Versions
   - 🧪 Beta Rust Channel

5. **Auto-approves and Merges** - If criteria are met:
   - Adds approval review with detailed reasoning
   - Enables auto-merge with squash method
   - PR is automatically merged when all checks pass

6. **Comments for Manual Review** - If manual review is required:
   - Adds comment explaining why manual review is needed
   - Provides guidance for maintainers

## Status Check Requirements

For auto-merge to proceed, the following checks must pass:

| Check | Status | Notes |
|-------|--------|-------|
| ✅ Check Code Quality | ✅ Must Pass | Format, lint, tests, spectral validation |
| 🔒 Security Audit | ⚠️ Can Fail | Security audits may fail for known issues |
| 🦀 MSRV | ✅ Must Pass | Minimum Supported Rust Version compatibility |
| 📦 Minimal Versions | ✅ Must Pass | Ensures compatibility with minimal dependency versions |
| 🧪 Beta Channel | ✅ Must Pass | Forward compatibility testing |

## Example Scenarios

### ✅ Auto-merged: Patch Update
```
chore(deps): bump serde from 1.0.195 to 1.0.196
```
- **Analysis**: Patch update (1.0.195 → 1.0.196)
- **Action**: Automatically approved and merged
- **Reason**: Patch updates are safe and backward compatible

### ✅ Auto-merged: Security Update
```
chore(deps): bump openssl from 0.10.60 to 0.10.61 [SECURITY]
```
- **Analysis**: Minor update but contains security fix
- **Action**: Automatically approved and merged
- **Reason**: Security updates are prioritized regardless of version type

### ⚠️ Manual Review: Minor Update
```
chore(deps): bump tokio from 1.35.0 to 1.36.0
```
- **Analysis**: Minor update (1.35 → 1.36)
- **Action**: Manual review required
- **Reason**: Minor updates may introduce new features or behavior changes

### ⚠️ Manual Review: Major Update
```
chore(deps): bump clap from 4.4.0 to 5.0.0
```
- **Analysis**: Major update (4.x → 5.x)
- **Action**: Manual review required
- **Reason**: Major updates likely contain breaking changes

## Benefits

### 🚀 **Faster Security Updates**
- Critical security patches are applied within hours
- Reduces vulnerability exposure time
- Maintains security posture automatically

### ⚡ **Reduced Maintenance Overhead**
- Eliminates manual intervention for routine updates
- Frees maintainer time for feature development
- Keeps dependencies current automatically

### 🛡️ **Quality Assurance**
- All updates must pass comprehensive CI checks
- Maintains code quality and compatibility
- Prevents regressions through automated testing

### 📊 **Transparency**
- Clear documentation of what gets auto-merged
- Detailed reasoning in PR comments
- Full audit trail of automated decisions

## Monitoring and Override

### Monitoring Auto-merge Activity

- Check the [Actions tab](https://github.com/ilaborie/clawspec/actions/workflows/dependabot-auto-merge.yml) for workflow runs
- Review auto-merged PRs in the [Pull Requests](https://github.com/ilaborie/clawspec/pulls?q=is%3Apr+author%3Adependabot) history
- Monitor for any failed auto-merge attempts

### Manual Override

If an auto-merge is inappropriate:

1. **Disable auto-merge**: Go to the PR and click "Disable auto-merge"
2. **Close and recreate**: Close the PR and let Dependabot recreate it
3. **Manual review**: Add the PR to manual review queue

### Emergency Stops

To temporarily disable auto-merge:

1. **Repository level**: Disable the "Dependabot Auto-merge" workflow in repository settings
2. **Workflow level**: Edit `.github/workflows/dependabot-auto-merge.yml` and add condition `if: false`

## Configuration Updates

When modifying the auto-merge behavior:

1. **Test thoroughly**: Use a fork or feature branch to test changes
2. **Update documentation**: Keep this file synchronized with configuration changes  
3. **Monitor carefully**: Watch the first few auto-merges after configuration changes
4. **Iterate gradually**: Start with conservative settings and expand as confidence grows

## Support

For questions or issues with the auto-merge system:

1. **Check logs**: Review workflow runs in the Actions tab
2. **File issue**: Create an issue with the `dependencies` label
3. **Manual intervention**: Maintainers can always manually review and merge any PR

---

*This configuration balances automation with safety, ensuring rapid deployment of routine updates while maintaining careful oversight of potentially impactful changes.*