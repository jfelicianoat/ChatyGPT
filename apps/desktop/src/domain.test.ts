import { describe, expect, it } from "vitest";
import {
  canSendMessage,
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  type LocalTaskSnapshot
} from "./domain";

const task = (remoteStatus: string, localState = "polling"): LocalTaskSnapshot => ({
  id: "local-test",
  remoteStatus,
  localState,
  consecutivePollErrors: 0,
  pendingToolCalls: [],
  updatedAt: "2026-07-20T00:00:00Z"
});

describe("message submission eligibility", () => {
  it("does not depend on running the optional broker diagnostic", () => {
    expect(canSendMessage({
      hasConversation: true,
      hasText: true,
      attachmentsReady: true,
      attachmentBusy: false,
      turnBlocking: false
    })).toBe(true);
  });

  it("blocks both click and keyboard submission while local prerequisites are pending", () => {
    expect(canSendMessage({
      hasConversation: true,
      hasText: true,
      attachmentsReady: false,
      attachmentBusy: false,
      turnBlocking: false
    })).toBe(false);
  });
});

describe("broker task state helpers", () => {
  it("keeps a generating task blocking and pollable", () => {
    expect(isTerminalTask(task("generating"))).toBe(false);
    expect(isTaskPollingComplete(task("generating"))).toBe(false);
    expect(isTaskBlockingConversation(task("generating"))).toBe(true);
  });

  it("recognizes terminal and orphaned tasks", () => {
    expect(isTerminalTask(task("completed", "terminal"))).toBe(true);
    expect(isTaskPollingComplete(task("failed", "terminal"))).toBe(true);
    expect(isTaskBlockingConversation(task("not_submitted", "orphaned"))).toBe(false);
  });
});
