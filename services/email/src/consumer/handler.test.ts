import "../test/setup.ts";

import { expect } from "@std/expect";
import { assertRejects } from "@std/assert";
import { type Spy, spy } from "@std/testing/mock";
import type { Resend } from "resend";
import type { SendEmailParams, SendResult } from "../sender/resend.ts";
import { handleAuthEmailMessage, type HandlerDependencies } from "./handler.ts";
import {
  createEmailVerificationMessage,
  createMessageMetadata,
  createPasswordResetMessage,
} from "../test/fixtures/messages.ts";

// deno-lint-ignore no-explicit-any
type AnySpy = Spy<any, any[], any>;

function createMockDeps(sendEmailResult: SendResult | Error): {
  deps: HandlerDependencies;
  sendEmailSpy: AnySpy;
  recordEmailSentSpy: AnySpy;
  recordEmailFailedSpy: AnySpy;
} {
  const sendEmailSpy = spy(
    sendEmailResult instanceof Error
      ? () => Promise.reject(sendEmailResult)
      : () => Promise.resolve(sendEmailResult),
  );

  const recordEmailSentSpy = spy((_emailType: string, _durationMs: number) => {});
  const recordEmailFailedSpy = spy(
    (_emailType: string, _errorType: string, _durationMs: number) => {},
  );

  const deps: HandlerDependencies = {
    sendEmail: sendEmailSpy as unknown as HandlerDependencies["sendEmail"],
    recordEmailSent: recordEmailSentSpy,
    recordEmailFailed: recordEmailFailedSpy,
    resendClient: {} as Resend,
  };

  return { deps, sendEmailSpy, recordEmailSentSpy, recordEmailFailedSpy };
}

Deno.test("handleAuthEmailMessage", async (t) => {
  await t.step("successful send records metrics and returns normally", async () => {
    const { deps, sendEmailSpy, recordEmailSentSpy, recordEmailFailedSpy } = createMockDeps({
      success: true,
      messageId: "msg_123",
    });
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    await handleAuthEmailMessage(message, metadata, deps);

    expect(sendEmailSpy.calls.length).toBe(1);
    expect(recordEmailSentSpy.calls.length).toBe(1);
    expect(recordEmailSentSpy.calls[0].args[0]).toBe("emailVerification");
    expect(recordEmailFailedSpy.calls.length).toBe(0);
  });

  await t.step("retryable API error throws and records failure metrics", async () => {
    const { deps, recordEmailSentSpy, recordEmailFailedSpy } = createMockDeps({
      success: false,
      error: "Rate limited",
    });
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    await assertRejects(
      () => handleAuthEmailMessage(message, metadata, deps),
      Error,
      "Rate limited",
    );

    expect(recordEmailFailedSpy.calls.length).toBe(1);
    expect(recordEmailFailedSpy.calls[0].args[0]).toBe("emailVerification");
    // Error type is "Error" because the catch block records the thrown error's constructor name
    expect(recordEmailFailedSpy.calls[0].args[1]).toBe("Error");
    expect(recordEmailSentSpy.calls.length).toBe(0);
  });

  await t.step("non-retryable API error returns without throwing", async () => {
    const { deps, recordEmailFailedSpy } = createMockDeps({
      success: false,
      error: "Invalid email address",
    });
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    // Should not throw
    await handleAuthEmailMessage(message, metadata, deps);

    expect(recordEmailFailedSpy.calls.length).toBe(1);
    expect(recordEmailFailedSpy.calls[0].args[0]).toBe("emailVerification");
    expect(recordEmailFailedSpy.calls[0].args[1]).toBe("Invalid email address");
  });

  await t.step("retryable exception throws and records failure metrics", async () => {
    const { deps, recordEmailSentSpy, recordEmailFailedSpy } = createMockDeps(
      new Error("Connection refused"),
    );
    const message = createPasswordResetMessage();
    const metadata = createMessageMetadata();

    await assertRejects(
      () => handleAuthEmailMessage(message, metadata, deps),
      Error,
      "Connection refused",
    );

    expect(recordEmailFailedSpy.calls.length).toBe(1);
    expect(recordEmailFailedSpy.calls[0].args[0]).toBe("passwordReset");
    expect(recordEmailFailedSpy.calls[0].args[1]).toBe("Error");
    expect(recordEmailSentSpy.calls.length).toBe(0);
  });

  await t.step("non-retryable exception returns without throwing", async () => {
    const { deps, recordEmailFailedSpy } = createMockDeps(
      new Error("validation_error: bad format"),
    );
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    // Should not throw
    await handleAuthEmailMessage(message, metadata, deps);

    expect(recordEmailFailedSpy.calls.length).toBe(1);
  });

  await t.step("passes correct parameters to sendEmail", async () => {
    const { deps, sendEmailSpy } = createMockDeps({
      success: true,
      messageId: "msg_123",
    });
    const message = createEmailVerificationMessage(
      { verificationCode: "TEST123" },
      { recipientEmail: "user@test.com", idempotencyKey: "key-abc" },
    );
    const metadata = createMessageMetadata();

    await handleAuthEmailMessage(message, metadata, deps);

    const params = sendEmailSpy.calls[0].args[0] as SendEmailParams;
    expect(params.to).toBe("user@test.com");
    expect(params.subject).toBe("Verify your email address");
    expect(params.idempotencyKey).toBe("key-abc");
  });

  await t.step("handles Domain not allowed as non-retryable", async () => {
    const { deps, recordEmailFailedSpy } = createMockDeps({
      success: false,
      error: "Domain not allowed",
    });
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    // Should not throw
    await handleAuthEmailMessage(message, metadata, deps);

    expect(recordEmailFailedSpy.calls.length).toBe(1);
  });

  await t.step("handles Missing email payload as non-retryable", async () => {
    const { deps, recordEmailFailedSpy } = createMockDeps(
      new Error("Missing email payload"),
    );
    const message = createEmailVerificationMessage();
    const metadata = createMessageMetadata();

    // Should not throw
    await handleAuthEmailMessage(message, metadata, deps);

    expect(recordEmailFailedSpy.calls.length).toBe(1);
  });
});
