import "../test/setup.ts";

import { expect } from "@std/expect";
import { assertThrows } from "@std/assert";
import { isValidElement } from "react";
import {
  createBaseMessage,
  createChangeEmailMessage,
  createEmailVerificationMessage,
  createPasswordChangedMessage,
  createPasswordResetMessage,
  createTwoFactorEnabledMessage,
  createTwoFactorSetupMessage,
} from "../test/fixtures/messages.ts";
import { getAuthEmailContent } from "./index.tsx";

Deno.test("getAuthEmailContent", async (t) => {
  await t.step("emailVerification returns correct subject and react element", () => {
    const message = createEmailVerificationMessage({ verificationCode: "ABC123" });

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Verify your email address");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("passwordReset returns correct subject and react element", () => {
    const message = createPasswordResetMessage({ resetCode: "RESET456" });

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Reset your password");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("changeEmail returns correct subject and react element", () => {
    const message = createChangeEmailMessage({ confirmationCode: "CONFIRM789" });

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Confirm your new email address");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("passwordChanged returns correct subject and react element", () => {
    const message = createPasswordChangedMessage();

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Your password was changed");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("twoFactorSetup returns correct subject and react element", () => {
    const message = createTwoFactorSetupMessage({ setupCode: "2FA-SETUP" });

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Complete your two-factor authentication setup");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("twoFactorEnabled returns correct subject and react element", () => {
    const message = createTwoFactorEnabledMessage();

    const result = getAuthEmailContent(message);

    expect(result.subject).toBe("Two-factor authentication is now enabled");
    expect(isValidElement(result.react)).toBe(true);
  });

  await t.step("throws error when payload is missing", () => {
    const message = createBaseMessage({ payload: undefined });

    assertThrows(
      () => getAuthEmailContent(message),
      Error,
      "Missing email payload",
    );
  });
});
