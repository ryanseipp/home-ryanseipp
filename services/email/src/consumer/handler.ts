import type { AuthEmailMessage } from "../generated/ryanseipp/email/v1/auth.ts";
import {
  recordEmailFailed as defaultRecordEmailFailed,
  recordEmailSent as defaultRecordEmailSent,
} from "../observability/metrics.ts";
import {
  resendClient,
  sendEmail as defaultSendEmail,
  type SendEmailParams,
  type SendResult,
} from "../sender/resend.ts";
import { getAuthEmailContent } from "../templates/index.tsx";
import type { Resend } from "resend";

export interface MessageMetadata {
  topic: string;
  partition: number;
  offset: string;
}

export interface HandlerDependencies {
  sendEmail: (params: SendEmailParams, client: Resend) => Promise<SendResult>;
  recordEmailSent: (emailType: string, durationMs: number) => void;
  recordEmailFailed: (
    emailType: string,
    errorType: string,
    durationMs: number,
  ) => void;
  resendClient: Resend;
}

const defaultDependencies: HandlerDependencies = {
  sendEmail: defaultSendEmail,
  recordEmailSent: defaultRecordEmailSent,
  recordEmailFailed: defaultRecordEmailFailed,
  resendClient,
};

// Errors that should not be retried
const NON_RETRYABLE_ERRORS = [
  "Invalid email address",
  "Domain not allowed",
  "Unknown email payload type",
  "Missing email payload",
  "validation_error",
];

function isRetryable(error: Error): boolean {
  return !NON_RETRYABLE_ERRORS.some((msg) => error.message.includes(msg));
}

function getEmailType(message: AuthEmailMessage): string {
  return message.payload?.$case ?? "unknown";
}

export async function handleAuthEmailMessage(
  message: AuthEmailMessage,
  metadata: MessageMetadata,
  deps: HandlerDependencies = defaultDependencies,
): Promise<void> {
  const { idempotencyKey, recipientEmail, traceId, spanId } = message;
  const emailType = getEmailType(message);
  const sendStartTime = Date.now();
  const ctx = { traceId, spanId, idempotencyKey, emailType };

  try {
    // 1. Get email content (React element + subject)
    const content = getAuthEmailContent(message);
    console.debug("Email content prepared", {
      ...ctx,
      subject: content.subject,
    });

    // 2. Send via Resend (renders React to HTML internally)
    const result = await deps.sendEmail(
      {
        to: recipientEmail,
        subject: content.subject,
        react: content.react,
        idempotencyKey,
        traceId,
      },
      deps.resendClient,
    );

    const durationMs = Date.now() - sendStartTime;

    if (!result.success) {
      const error = new Error(result.error ?? "Email send failed");

      if (!isRetryable(error)) {
        // Log and skip - don't retry
        deps.recordEmailFailed(
          emailType,
          result.error ?? "unknown",
          durationMs,
        );
        console.error("Non-retryable email error, skipping", {
          ...ctx,
          error: result.error,
        });
        return;
      }

      // For retryable errors, let the catch block handle metrics
      throw error; // Retry via KafkaJS
    }

    deps.recordEmailSent(emailType, durationMs);
    console.info("Email processed successfully", {
      ...ctx,
      messageId: result.messageId,
      durationMs,
      ...metadata,
    });
  } catch (error) {
    const durationMs = Date.now() - sendStartTime;
    const errorType = error instanceof Error ? error.constructor.name : "unknown";
    deps.recordEmailFailed(emailType, errorType, durationMs);

    if (error instanceof Error && !isRetryable(error)) {
      console.error("Non-retryable error, skipping message", {
        ...ctx,
        error: error.message,
      });
      return;
    }

    throw error; // Propagate for retry
  }
}
