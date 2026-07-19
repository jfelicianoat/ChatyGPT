import { invoke } from "@tauri-apps/api/core";
import type { BootstrapReport, BrokerDiagnostic } from "./domain";

export const platform = {
  bootstrap(): Promise<BootstrapReport> {
    return invoke<BootstrapReport>("bootstrap_app");
  },
  diagnoseBroker(): Promise<BrokerDiagnostic> {
    return invoke<BrokerDiagnostic>("diagnose_broker");
  }
};

