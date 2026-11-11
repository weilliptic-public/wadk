/*
 * Copyright 2026 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Modified by Weilliptic: ported to the Weil ADK examples, on any model, with
 * every step audited under the agent's wallet.
 */

package com.weilliptic.weilwallet.examples.financialadvisor;

import com.google.adk.agents.LlmAgent;
import com.google.adk.models.BaseLlm;

import com.weilliptic.weilwallet.adk.WeilAgentSetup;

/**
 * The model shared by every agent: the one MODEL_PROVIDER picks ({@link WeilAgentSetup#model()}),
 * where the recipe reads a Gemini MODEL_NAME from .env.
 */
final class Models {

  private static final BaseLlm MODEL = WeilAgentSetup.model();

  private Models() {}

  /** Returns an agent builder with the shared model already set. */
  static LlmAgent.Builder agentBuilder() {
    return LlmAgent.builder().model(MODEL);
  }
}
