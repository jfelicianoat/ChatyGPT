import { describe, expect, it } from "vitest";
import {
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
  updatedAt: "2026-07-20T00:00:00Z"
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
