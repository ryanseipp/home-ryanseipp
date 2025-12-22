import { type Consumer, type EachMessagePayload, Kafka, logLevel, type SASLOptions } from "kafkajs";
import { config } from "../config.ts";
import { AuthEmailMessage } from "../generated/ryanseipp/email/v1/auth.ts";
import { handleAuthEmailMessage } from "./handler.ts";

let consumer: Consumer | undefined;

export async function startConsumer(): Promise<void> {
  const kafka = new Kafka({
    clientId: config.kafka.clientId,
    brokers: config.kafka.brokers,
    ssl: config.kafka.ssl,
    sasl: config.kafka.sasl as SASLOptions | undefined,
    logLevel: logLevel.INFO,
    logCreator: () => ({ namespace, level, log }) => {
      const { message, ...extra } = log;
      console.info(`[kafka] ${message}`, {
        namespace,
        kafkaLevel: level,
        ...extra,
      });
    },
  });

  consumer = kafka.consumer({
    groupId: config.kafka.groupId,
    sessionTimeout: 30000,
    heartbeatInterval: 3000,
  });

  await consumer.connect();
  console.info("Kafka consumer connected");

  await consumer.subscribe({
    topic: "email.auth",
    fromBeginning: false,
  });

  await consumer.run({
    eachMessage: async (payload: EachMessagePayload) => {
      const { topic, partition, message } = payload;
      const startTime = Date.now();

      try {
        if (!message.value) {
          console.warn("Empty message received", {
            topic,
            partition,
            offset: message.offset,
          });
          return;
        }

        const authMessage = AuthEmailMessage.decode(message.value);

        await handleAuthEmailMessage(authMessage, {
          topic,
          partition,
          offset: message.offset,
        });

        const durationMs = Date.now() - startTime;

        console.info("Message processed successfully", {
          topic,
          partition,
          offset: message.offset,
          durationMs,
          traceId: authMessage.traceId,
        });
      } catch (error) {
        const durationMs = Date.now() - startTime;

        console.error("Failed to process message", {
          topic,
          partition,
          offset: message.offset,
          error: error instanceof Error ? error.message : "Unknown error",
          durationMs,
        });

        // Let KafkaJS handle retry based on consumer config
        throw error;
      }
    },
  });

  console.info("Consumer started", {
    topic: "email.auth",
    groupId: config.kafka.groupId,
  });
}

export async function stopConsumer(): Promise<void> {
  if (consumer) {
    await consumer.disconnect();
    console.info("Kafka consumer disconnected");
  }
}
