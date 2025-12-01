import "../test/setup.ts";

import { expect } from "@std/expect";
import { stub } from "@std/testing/mock";
import type { Resend } from "resend";
import { sendEmail } from "./resend.ts";

function createMockResendClient() {
  const sendStub = stub(
    { send: () => {} },
    "send",
    () => Promise.resolve({ data: { id: "msg_123" }, error: null }),
  );

  const client = {
    emails: { send: sendStub },
  } as unknown as Resend;

  return { client, sendStub };
}

const baseParams = {
  to: "test@example.com",
  subject: "Test Subject",
  react: <div>Test content</div>,
  idempotencyKey: "test-key-123",
  traceId: "trace-456",
};

Deno.test("sendEmail", async (t) => {
  await t.step("returns success with messageId when API succeeds", async () => {
    const { client, sendStub } = createMockResendClient();

    const result = await sendEmail(baseParams, client);

    expect(result).toEqual({
      success: true,
      messageId: "msg_123",
    });
    expect(sendStub.calls.length).toBe(1);
  });

  await t.step("returns error when API returns error response", async () => {
    const sendStub = stub(
      { send: () => {} },
      "send",
      () => Promise.resolve({ data: null, error: { message: "Invalid email address" } }),
    );
    const client = { emails: { send: sendStub } } as unknown as Resend;

    const result = await sendEmail(baseParams, client);

    expect(result).toEqual({
      success: false,
      error: "Invalid email address",
    });
  });

  await t.step("returns error when API throws exception", async () => {
    const sendStub = stub(
      { send: () => {} },
      "send",
      () => Promise.reject(new Error("Network timeout")),
    );
    const client = { emails: { send: sendStub } } as unknown as Resend;

    const result = await sendEmail(baseParams, client);

    expect(result).toEqual({
      success: false,
      error: "Network timeout",
    });
  });

  await t.step("handles non-Error exceptions", async () => {
    const sendStub = stub(
      { send: () => {} },
      "send",
      () => Promise.reject("string error"),
    );
    const client = { emails: { send: sendStub } } as unknown as Resend;

    const result = await sendEmail(baseParams, client);

    expect(result).toEqual({
      success: false,
      error: "Unknown error",
    });
  });

  await t.step("passes idempotencyKey in headers and options", async () => {
    const { client, sendStub } = createMockResendClient();

    await sendEmail(baseParams, client);

    // deno-lint-ignore no-explicit-any
    const args = sendStub.calls[0].args as any[];
    const emailData = args[0];
    const options = args[1];
    expect(emailData.headers["X-Entity-Ref-ID"]).toBe("test-key-123");
    expect(options.idempotencyKey).toBe("test-key-123");
  });

  await t.step("passes email parameters correctly", async () => {
    const { client, sendStub } = createMockResendClient();

    await sendEmail(baseParams, client);

    // deno-lint-ignore no-explicit-any
    const args = sendStub.calls[0].args as any[];
    const emailData = args[0];
    expect(emailData.to).toBe("test@example.com");
    expect(emailData.subject).toBe("Test Subject");
    expect(emailData.react).toBeDefined();
  });
});
