import type { ReactElement } from "react";
import { Resend } from "resend";
import { config } from "../config.ts";

export const resendClient = new Resend(config.resend.apiKey);

export interface SendEmailParams {
  to: string;
  subject: string;
  react: ReactElement;
  idempotencyKey: string;
  traceId?: string;
}

export interface SendResult {
  success: boolean;
  messageId?: string;
  error?: string;
}

export async function sendEmail(
  params: SendEmailParams,
  client: Resend,
): Promise<SendResult> {
  const { to, subject, react, idempotencyKey, traceId } = params;

  try {
    const result = await client.emails.send(
      {
        from: config.resend.fromEmail,
        to,
        subject,
        react,
        headers: {
          "X-Entity-Ref-ID": idempotencyKey,
        },
      },
      {
        idempotencyKey,
      },
    );

    if (result.error) {
      console.error("Resend API returned error", {
        to,
        subject,
        idempotencyKey,
        traceId,
        error: result.error.message,
      });

      return {
        success: false,
        error: result.error.message,
      };
    }

    console.info("Email sent successfully", {
      to,
      subject,
      idempotencyKey,
      traceId,
      messageId: result.data?.id,
    });

    return {
      success: true,
      messageId: result.data?.id,
    };
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : "Unknown error";

    console.error("Failed to send email via Resend", {
      to,
      subject,
      idempotencyKey,
      traceId,
      error: errorMessage,
    });

    return {
      success: false,
      error: errorMessage,
    };
  }
}
