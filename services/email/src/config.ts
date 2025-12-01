import { z } from "zod";

const envSchema = z.object({
  // Kafka
  KAFKA_BROKERS: z.string().transform((s) => s.split(",")),
  KAFKA_CLIENT_ID: z.string().default("email-service"),
  KAFKA_GROUP_ID: z.string().default("email-service-consumer"),
  KAFKA_SSL: z
    .string()
    .default("true")
    .transform((s) => s !== "false"),
  KAFKA_SASL_MECHANISM: z
    .enum(["plain", "scram-sha-256", "scram-sha-512"])
    .default("scram-sha-512")
    .optional(),
  KAFKA_SASL_USERNAME: z.string().optional(),
  KAFKA_SASL_PASSWORD: z.string().optional(),

  // Resend
  RESEND_API_KEY: z.string().min(1),
  RESEND_FROM_EMAIL: z.string().email().default("noreply@ryanseipp.com"),

  // OpenTelemetry (endpoint now handled by OTEL_EXPORTER_OTLP_ENDPOINT env var directly)
  OTEL_SERVICE_NAME: z.string().default("email-service"),

  // Logging
  LOG_LEVEL: z.enum(["trace", "debug", "info", "warn", "error"]).default("info"),
});

const parsed = envSchema.safeParse(Deno.env.toObject());
if (!parsed.success) {
  console.error("Invalid environment variables:", parsed.error.flatten().fieldErrors);
  Deno.exit(1);
}

const env = parsed.data;

// Maintain existing config access patterns for consumers
export const config = {
  kafka: {
    brokers: env.KAFKA_BROKERS,
    clientId: env.KAFKA_CLIENT_ID,
    groupId: env.KAFKA_GROUP_ID,
    ssl: env.KAFKA_SSL,
    sasl: env.KAFKA_SASL_USERNAME
      ? {
        mechanism: env.KAFKA_SASL_MECHANISM,
        username: env.KAFKA_SASL_USERNAME,
        password: env.KAFKA_SASL_PASSWORD,
      }
      : undefined,
  },
  resend: {
    apiKey: env.RESEND_API_KEY,
    fromEmail: env.RESEND_FROM_EMAIL,
  },
  otel: {
    serviceName: env.OTEL_SERVICE_NAME,
  },
  logLevel: env.LOG_LEVEL,
};

export type Config = typeof config;
