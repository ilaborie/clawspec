# Security Policy

## Supported Versions

Security updates are provided for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security vulnerability in Clawspec, please follow these steps:

### Private Disclosure

**Do NOT create a public GitHub issue for security vulnerabilities.**

Instead, please report security vulnerabilities privately using one of these methods:

1. **GitHub Security Advisories** (Preferred)
   - Go to the [Security tab](https://github.com/ilaborie/clawspec/security)
   - Click "Report a vulnerability"
   - Fill out the vulnerability report form

2. **Email**
   - Send an email to: [security@clawspec.dev] (if available)
   - Include "SECURITY" in the subject line

### What to Include

When reporting a security vulnerability, please include:

- **Description** of the vulnerability
- **Steps to reproduce** the issue
- **Potential impact** and attack scenarios
- **Suggested fix** (if you have one)
- **Your contact information** for follow-up

### Response Process

1. **Acknowledgment**: We will acknowledge receipt within 48 hours
2. **Investigation**: We will investigate and assess the vulnerability
3. **Updates**: We will provide updates on our progress within 7 days
4. **Resolution**: We aim to resolve critical vulnerabilities within 30 days

### Disclosure Timeline

- **Immediate**: Fix critical vulnerabilities affecting production systems
- **7 days**: Provide initial assessment and timeline
- **30 days**: Target resolution for most vulnerabilities
- **90 days**: Maximum time before public disclosure (coordinated disclosure)

## Security Measures

### Development Security

- **Dependency Scanning**: Automated security audits with `cargo audit`
- **Code Review**: All changes require maintainer review
- **CI/CD Security**: Automated security checks in GitHub Actions
- **Dependency Updates**: Regular updates via Dependabot

### Supply Chain Security

- **Verified Dependencies**: Only use well-maintained, audited crates
- **Lock Files**: Cargo.lock committed to ensure reproducible builds
- **Minimal Dependencies**: Keep dependency tree as small as possible
- **Security Audits**: Regular audits of the dependency chain

### Runtime Security

- **Memory Safety**: Rust's memory safety prevents many common vulnerabilities
- **No Unsafe Code**: Policy against unsafe code (enforced by clippy)
- **Input Validation**: Proper validation of user inputs
- **Error Handling**: No sensitive information in error messages

## Security Best Practices for Users

### General Usage

- **Keep Updated**: Always use the latest version of Clawspec
- **Audit Dependencies**: Regular security audits of your project dependencies
- **Secure CI/CD**: Ensure your CI/CD pipelines are secure
- **Access Control**: Limit access to OpenAPI generation in production

### OpenAPI Security

- **Sensitive Data**: Never include sensitive data in OpenAPI specs
- **Access Control**: Implement proper authentication for generated APIs
- **Rate Limiting**: Apply rate limiting to generated endpoints
- **Input Validation**: Validate all inputs on your API endpoints

## Vulnerability Database

We maintain awareness of security issues in:

- **Rust Security Database**: Monitor RustSec advisories
- **CVE Database**: Track Common Vulnerabilities and Exposures
- **GitHub Security Advisories**: Monitor ecosystem vulnerabilities
- **Dependency Advisories**: Track security issues in dependencies

## Security Updates

Security updates will be:

- **Released promptly** for critical vulnerabilities
- **Clearly documented** in release notes
- **Backward compatible** when possible
- **Accompanied by migration guides** for breaking security fixes

## Contact

For security-related questions or concerns:

- **Security Reports**: Use GitHub Security Advisories
- **General Security Questions**: Create a GitHub Discussion
- **Emergency Contact**: [Provide emergency contact if needed]

## Acknowledgments

We appreciate security researchers and users who help improve the security of Clawspec by responsibly disclosing vulnerabilities.

Security contributors will be acknowledged in:
- Security advisories (unless anonymity is requested)
- Release notes for security fixes
- Security hall of fame (if implemented)

---

**Thank you for helping keep Clawspec secure!** 🔒