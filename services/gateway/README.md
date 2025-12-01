# Gateway

A Gateway API for home.ryanseipp.com, fronting all other microservices within
the project. Manages session-based authentication, and authenticates to other
microservices via mTLS and JWTs received from the identity service.

Downstream services are expected to authenticate user requests via JWT, and
JWKS, and perform their own authorization. All downstream communication should
occur over gRPC unless this isn't possible in the desired language's ecosystem.

This service stores minimal information about user preferences, sessions, and
JWT tokens in a NoSQL ScyllaDB database.
