# Identity Service - Relevant Specifications

## OAuth 2.1

OAuth 2.1 consolidates and updates the OAuth 2.0 authorization framework,
incorporating security best practices from several years of deployment
experience.

| Specification            | Title                                 | Link                                                    |
| ------------------------ | ------------------------------------- | ------------------------------------------------------- |
| draft-ietf-oauth-v2-1-14 | The OAuth 2.1 Authorization Framework | https://datatracker.ietf.org/doc/draft-ietf-oauth-v2-1/ |

> **Note:** OAuth 2.1 has not yet been published as a final RFC. It is an active
> Internet-Draft (last updated 2025-10-19).

### Consolidated OAuth 2.0 RFCs

OAuth 2.1 incorporates and supersedes the following specifications:

| RFC      | Title                                                      | Link                                          |
| -------- | ---------------------------------------------------------- | --------------------------------------------- |
| RFC 6749 | The OAuth 2.0 Authorization Framework                      | https://datatracker.ietf.org/doc/html/rfc6749 |
| RFC 6750 | The OAuth 2.0 Authorization Framework: Bearer Token Usage  | https://datatracker.ietf.org/doc/html/rfc6750 |
| RFC 7636 | Proof Key for Code Exchange by OAuth Public Clients (PKCE) | https://datatracker.ietf.org/doc/html/rfc7636 |
| RFC 8252 | OAuth 2.0 for Native Apps                                  | https://datatracker.ietf.org/doc/html/rfc8252 |
| RFC 9207 | OAuth 2.0 Authorization Server Issuer Identification       | https://datatracker.ietf.org/doc/html/rfc9207 |

## OpenID Connect (OIDC)

OpenID Connect 1.0 is an identity layer on top of the OAuth 2.0 protocol. These
specifications are maintained by the OpenID Foundation, not as IETF RFCs.

| Specification                                  | Title                                                                                                        | Link                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| OpenID Connect Core 1.0                        | Authentication built on top of OAuth 2.0 and the use of Claims to communicate information about the End-User | https://openid.net/specs/openid-connect-core-1_0.html         |
| OpenID Connect Discovery 1.0                   | How clients dynamically discover information about OpenID Providers                                          | https://openid.net/specs/openid-connect-discovery-1_0.html    |
| OpenID Connect Dynamic Client Registration 1.0 | How clients dynamically register with OpenID Providers                                                       | https://openid.net/specs/openid-connect-registration-1_0.html |
| OpenID Connect RP-Initiated Logout 1.0         | How a Relying Party requests that an OpenID Provider log out the End-User                                    | https://openid.net/specs/openid-connect-rpinitiated-1_0.html  |
| OpenID Connect Session Management 1.0          | Session management using postMessage-based mechanisms                                                        | https://openid.net/specs/openid-connect-session-1_0.html      |
| OpenID Connect Front-Channel Logout 1.0        | Front-channel logout mechanism without OP iframe                                                             | https://openid.net/specs/openid-connect-frontchannel-1_0.html |
| OpenID Connect Back-Channel Logout 1.0         | Direct back-channel communication for logout                                                                 | https://openid.net/specs/openid-connect-backchannel-1_0.html  |

## JSON Web Key Set (JWKS)

| RFC      | Title              | Link                                          |
| -------- | ------------------ | --------------------------------------------- |
| RFC 7517 | JSON Web Key (JWK) | https://datatracker.ietf.org/doc/html/rfc7517 |

## JSON Web Token (JWT)

| RFC      | Title                                 | Link                                          |
| -------- | ------------------------------------- | --------------------------------------------- |
| RFC 7519 | JSON Web Token (JWT)                  | https://datatracker.ietf.org/doc/html/rfc7519 |
| RFC 7515 | JSON Web Signature (JWS)              | https://datatracker.ietf.org/doc/html/rfc7515 |
| RFC 7516 | JSON Web Encryption (JWE)             | https://datatracker.ietf.org/doc/html/rfc7516 |
| RFC 7518 | JSON Web Algorithms (JWA)             | https://datatracker.ietf.org/doc/html/rfc7518 |
| RFC 8725 | JSON Web Token Best Current Practices | https://datatracker.ietf.org/doc/html/rfc8725 |

## OWASP Cheat Sheet Series

### Directly Related

| Cheat Sheet             | Relevance                                                                                                             | Link                                                                                    |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| OAuth2                  | Security best practices for OAuth 2.0 flows, PKCE, token storage, sender-constrained tokens                           | https://cheatsheetseries.owasp.org/cheatsheets/OAuth2_Cheat_Sheet.html                  |
| JSON Web Token for Java | JWT signing, algorithm selection, `alg:none` attacks, token validation (language-agnostic guidance despite the title) | https://cheatsheetseries.owasp.org/cheatsheets/JSON_Web_Token_for_Java_Cheat_Sheet.html |
| Authentication          | Authentication mechanisms, session binding, credential storage, OpenID Connect guidance                               | https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html          |
| Session Management      | Session ID generation, lifecycle, fixation prevention, timeout policies                                               | https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html      |
| Key Management          | Key lifecycle, rotation, storage, and destruction practices                                                           | https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html          |

### Supporting

| Cheat Sheet                | Relevance                                                            | Link                                                                                       |
| -------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Cryptographic Storage      | Secure storage of keys and secrets, HSM/KMS guidance                 | https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html      |
| Secrets Management         | Handling secrets in applications, key/secret separation              | https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html         |
| Password Storage           | Hashing algorithms (Argon2id, bcrypt, scrypt), salting, work factors | https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html           |
| Forgot Password            | Secure password reset flows                                          | https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html            |
| Multifactor Authentication | MFA implementation guidance                                          | https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html |
| REST Security              | Securing REST APIs, JWT usage in API authentication                  | https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html              |
