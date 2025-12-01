# Identity

An Identity API for home.ryanseipp.com, managing validation of user
authentication via password, passkey, and 2FA, issuance of JWTs, and provides
user identity information such as name, email, etc. This is a tonic gRPC API
which communicates with a PostgreSQL database via SQLx.

This service is available in two different forms: Standard and FIPS. Standard
uses state-of-the-art hashing and encryption algorithms, while FIPS uses
algorithms that may be out of date, but are compliant with FIPS 140-2.
