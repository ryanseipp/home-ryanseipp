import type { ReactElement } from "react";
import { NotionMagicLinkEmail } from "../../emails/notion-magic-link.tsx";
import type { AuthEmailMessage } from "../generated/email/auth/v1/auth.ts";

export type { AuthEmailMessage };

export interface EmailContent {
  subject: string;
  react: ReactElement;
}

export function getAuthEmailContent(message: AuthEmailMessage): EmailContent {
  const payload = message.payload;

  if (!payload) {
    throw new Error("Missing email payload");
  }

  switch (payload.$case) {
    case "emailVerification": {
      const data = payload.emailVerification;
      console.debug("Preparing email template", { type: "email_verification" });

      return {
        subject: "Verify your email address",
        react: <NotionMagicLinkEmail loginCode={data.verificationCode} />,
      };
    }

    case "passwordReset": {
      const data = payload.passwordReset;
      console.debug("Preparing email template", { type: "password_reset" });

      return {
        subject: "Reset your password",
        react: <NotionMagicLinkEmail loginCode={data.resetCode} />,
      };
    }

    case "changeEmail": {
      const data = payload.changeEmail;
      console.debug("Preparing email template", { type: "change_email" });

      return {
        subject: "Confirm your new email address",
        react: <NotionMagicLinkEmail loginCode={data.confirmationCode} />,
      };
    }

    case "passwordChanged": {
      console.debug("Preparing email template", { type: "password_changed" });

      return {
        subject: "Your password was changed",
        react: <NotionMagicLinkEmail loginCode="Password Changed" />,
      };
    }

    case "twoFactorSetup": {
      const data = payload.twoFactorSetup;
      console.debug("Preparing email template", { type: "two_factor_setup" });

      return {
        subject: "Complete your two-factor authentication setup",
        react: <NotionMagicLinkEmail loginCode={data.setupCode} />,
      };
    }

    case "twoFactorEnabled": {
      console.debug("Preparing email template", { type: "two_factor_enabled" });

      return {
        subject: "Two-factor authentication is now enabled",
        react: <NotionMagicLinkEmail loginCode="2FA Enabled" />,
      };
    }

    default: {
      const _exhaustive: never = payload;
      throw new Error(`Unknown email payload type: ${_exhaustive}`);
    }
  }
}
