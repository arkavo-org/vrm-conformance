// Operation registry + dispatch for the babylon-vrm adapter.
//
// L1 + L2 state: every Phase 1 op and every reserved op returns a structured
// `Unimplemented` error. L3 will replace the Phase 1 entries with real
// implementations driven by [virtual-cast/babylon-vrm-loader] running in a
// Playwright Chromium WebGL2 context, mirroring how the three-vrm adapter is
// organized.

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

// AdapterContext is intentionally minimal at L2: there's no browser session
// yet, no Metal device, no in-memory model state. L3 grows this struct to
// carry a Playwright Page + a BabylonScene + a sessionId counter.
// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface AdapterContext {}

const PHASE_BY_METHOD: Record<string, string> = {
  // Phase 1 ops are declared but unimplemented until L3.
  load_vrm: "L3 (babylon-vrm integration deferred)",
  set_camera: "L3 (babylon-vrm integration deferred)",
  set_lighting: "L3 (babylon-vrm integration deferred)",
  set_post_processing: "L3 (babylon-vrm integration deferred)",
  render: "L3 (babylon-vrm integration deferred)",
  dispose: "L3 (babylon-vrm integration deferred)",
  // Reserved ops with phase labels per docs/operation-contract.md.
  set_environment: "v1.x",
  set_expression: "Phase 3",
  set_humanoid_pose: "Phase 2",
  set_root_transform: "Phase 2",
  animate_root_transform: "Phase 2",
  step_physics: "Phase 2",
  reset_physics: "Phase 2",
};

export interface DispatchSuccess<T = unknown> {
  ok: true;
  result: T;
}

export interface DispatchFailure {
  ok: false;
  error: RpcError;
}

export type DispatchOutcome<T = unknown> = DispatchSuccess<T> | DispatchFailure;

export async function dispatch(
  _ctx: AdapterContext,
  method: string,
  _params: unknown,
): Promise<DispatchOutcome> {
  const phase = PHASE_BY_METHOD[method];
  if (phase) {
    return {
      ok: false,
      error: {
        code: -32000,
        message: "Unimplemented",
        data: { phase },
      },
    };
  }
  return {
    ok: false,
    error: {
      code: -32601,
      message: `method not found: ${method}`,
    },
  };
}

export function knownMethods(): string[] {
  return Object.keys(PHASE_BY_METHOD);
}
