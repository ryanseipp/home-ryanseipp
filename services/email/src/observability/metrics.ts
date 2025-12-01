import { metrics } from "@opentelemetry/api";

const meter = metrics.getMeter("ryanseipp.email");

// Counters
const emailsSentCounter = meter.createCounter("ryanseipp.email.sent", {
  description: "Total number of emails sent successfully",
  unit: "{email}",
});

const emailsFailedCounter = meter.createCounter("ryanseipp.email.failed", {
  description: "Total number of emails that failed to send",
  unit: "{email}",
});

// Histograms - using seconds per OpenTelemetry semantic conventions
const emailSendDuration = meter.createHistogram("ryanseipp.email.send.duration", {
  description: "Duration of email send operations via Resend API",
  unit: "s",
});

// Helper functions for recording metrics with attributes
export function recordEmailSent(emailType: string, durationMs: number): void {
  const attributes = { "email.type": emailType };
  emailsSentCounter.add(1, attributes);
  emailSendDuration.record(durationMs / 1000, attributes);
}

export function recordEmailFailed(emailType: string, errorType: string, durationMs: number): void {
  const attributes = { "email.type": emailType, "error.type": errorType };
  emailsFailedCounter.add(1, attributes);
  emailSendDuration.record(durationMs / 1000, attributes);
}
