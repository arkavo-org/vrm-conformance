// Operation registry + dispatch. Until Phase 2C-b lands, every op returns
// Unimplemented with the appropriate phase label. Phase 1 ops report
// `v1.x` because they're scheduled to be implemented in 2C-b (the
// still-unbuilt next phase). Reserved Phase 2+ ops report their target
// phase.

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

const PHASE_BY_METHOD: Record<string, string> = {
  // Phase 1 ops — to be implemented in 2C-b
  load_vrm: "v1.x",
  set_camera: "v1.x",
  set_lighting: "v1.x",
  set_post_processing: "v1.x",
  render: "v1.x",
  dispose: "v1.x",

  // Reserved
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

/**
 * Dispatch one method invocation. In Phase 2C-a all known methods return
 * Unimplemented; the dispatch table exists to ensure unknown methods get
 * `-32601` while known-but-deferred methods get `-32000`.
 */
export function dispatch(method: string, _params: unknown): DispatchOutcome {
  const phase = PHASE_BY_METHOD[method];
  if (phase === undefined) {
    return {
      ok: false,
      error: {
        code: -32601,
        message: `method not found: ${method}`,
      },
    };
  }
  return {
    ok: false,
    error: {
      code: -32000,
      message: `${method}: not implemented in this adapter version`,
      data: { phase },
    },
  };
}

export function knownMethods(): string[] {
  return Object.keys(PHASE_BY_METHOD);
}
