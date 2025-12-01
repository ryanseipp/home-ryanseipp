import { startConsumer, stopConsumer } from "./consumer/index.ts";

async function main(): Promise<void> {
  // Deno's built-in OTEL is automatically enabled via OTEL_DENO=true and --unstable-otel flag
  // No manual initialization needed

  console.info("Starting email service");

  // Start Kafka consumer
  await startConsumer();

  console.info("Email service started successfully");
}

async function shutdown(signal: string): Promise<void> {
  console.info(`Received shutdown signal: ${signal}`);

  try {
    await stopConsumer();
    console.info("Graceful shutdown completed");
    Deno.exit(0);
  } catch (error) {
    console.error("Error during shutdown", error);
    Deno.exit(1);
  }
}

// Graceful shutdown handlers using Deno signal API
Deno.addSignalListener("SIGTERM", () => shutdown("SIGTERM"));
Deno.addSignalListener("SIGINT", () => shutdown("SIGINT"));

// Start the application
main().catch((error) => {
  console.error("Failed to start email service", error);
  Deno.exit(1);
});
