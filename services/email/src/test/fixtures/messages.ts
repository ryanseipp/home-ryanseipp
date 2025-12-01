import type { MessageMetadata } from "../../consumer/handler.ts";
import type {
  AuthEmailMessage,
  ChangeEmail,
  EmailVerification,
  PasswordChanged,
  PasswordReset,
  TwoFactorEnabled,
  TwoFactorSetup,
} from "../../generated/email/auth/v1/auth.ts";
import { AuthEmailMessage as AuthEmailMessageCodec } from "../../generated/email/auth/v1/auth.ts";

export function createMessageMetadata(overrides: Partial<MessageMetadata> = {}): MessageMetadata {
  return {
    topic: "auth.email.v1",
    partition: 0,
    offset: "123",
    ...overrides,
  };
}

export function createBaseMessage(overrides: Partial<AuthEmailMessage> = {}): AuthEmailMessage {
  return {
    idempotencyKey: "test-idempotency-key-123",
    recipientEmail: "test@example.com",
    recipientName: "Test User",
    traceId: "trace-123",
    spanId: "span-456",
    producedAtMs: Date.now(),
    payload: undefined,
    ...overrides,
  };
}

export function createEmailVerificationMessage(
  verificationOverrides: Partial<EmailVerification> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "emailVerification",
      emailVerification: {
        verificationCode: "123456",
        verificationLink: "https://example.com/verify?code=123456",
        expiresInMinutes: 30,
        ...verificationOverrides,
      },
    },
  });
}

export function createPasswordResetMessage(
  resetOverrides: Partial<PasswordReset> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "passwordReset",
      passwordReset: {
        resetCode: "reset-123",
        resetLink: "https://example.com/reset?code=reset-123",
        expiresInMinutes: 15,
        requestIp: "192.168.1.1",
        requestLocation: "San Francisco, CA",
        ...resetOverrides,
      },
    },
  });
}

export function createChangeEmailMessage(
  changeOverrides: Partial<ChangeEmail> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "changeEmail",
      changeEmail: {
        oldEmail: "old@example.com",
        confirmationCode: "confirm-456",
        confirmationLink: "https://example.com/confirm?code=confirm-456",
        expiresInMinutes: 60,
        ...changeOverrides,
      },
    },
  });
}

export function createPasswordChangedMessage(
  changedOverrides: Partial<PasswordChanged> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "passwordChanged",
      passwordChanged: {
        changedAtMs: Date.now(),
        changeIp: "10.0.0.1",
        changeLocation: "New York, NY",
        ...changedOverrides,
      },
    },
  });
}

export function createTwoFactorSetupMessage(
  setupOverrides: Partial<TwoFactorSetup> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "twoFactorSetup",
      twoFactorSetup: {
        setupCode: "2fa-setup-789",
        expiresInMinutes: 10,
        ...setupOverrides,
      },
    },
  });
}

export function createTwoFactorEnabledMessage(
  enabledOverrides: Partial<TwoFactorEnabled> = {},
  messageOverrides: Partial<AuthEmailMessage> = {},
): AuthEmailMessage {
  return createBaseMessage({
    ...messageOverrides,
    payload: {
      $case: "twoFactorEnabled",
      twoFactorEnabled: {
        enabledAtMs: Date.now(),
        backupCodesHint: "Store your backup codes safely",
        ...enabledOverrides,
      },
    },
  });
}

// Helper to encode message to binary (for Kafka tests)
export function encodeMessage(message: AuthEmailMessage): Uint8Array {
  return AuthEmailMessageCodec.encode(message).finish();
}
