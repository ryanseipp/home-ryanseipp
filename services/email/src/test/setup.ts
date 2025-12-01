// Test setup for Deno
// Set required env vars for config module
Deno.env.set("KAFKA_BROKERS", "localhost:9092");
Deno.env.set("RESEND_API_KEY", "re_test_key");
Deno.env.set("LOG_LEVEL", "error"); // Silence logs during tests
