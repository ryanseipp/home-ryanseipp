package com.ryanseipp.home;

import static org.junit.jupiter.api.Assertions.assertEquals;

import io.quarkus.test.junit.QuarkusTest;
import jakarta.inject.Inject;
import org.eclipse.microprofile.reactive.messaging.Message;
import org.junit.jupiter.api.Test;

@QuarkusTest
class MyMessagingApplicationTest {

  @Inject MyMessagingApplication application;

  @Test
  void test() {
    assertEquals("HELLO", application.toUpperCase(Message.of("Hello")).getPayload());
    assertEquals("BONJOUR", application.toUpperCase(Message.of("bonjour")).getPayload());
  }
}
