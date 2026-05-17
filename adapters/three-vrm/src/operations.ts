// Operation registry + dispatch. Phase 2C-b: load_vrm, set_*, render,
// dispose call into the browser session; reserved Phase 2+ ops still
// return Unimplemented with the appropriate phase label.

import type { BrowserSession } from "./browser-session.js";

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface AdapterContext {
  session: BrowserSession;
  /** session_id counter. v0.1 is single-session: load_vrm overwrites
   *  whatever was loaded prior; the session_id is purely for protocol
   *  compliance. */
  nextSessionId: { value: number };
}

const PHASE_BY_RESERVED_METHOD: Record<string, string> = {
  set_environment: "v1.x",
  set_expression: "Phase 3",
  set_humanoid_pose: "Phase 2",
  set_root_transform: "Phase 2",
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
  ctx: AdapterContext,
  method: string,
  params: unknown,
): Promise<DispatchOutcome> {
  try {
    switch (method) {
      case "load_vrm": {
        const p = params as { path?: string };
        if (!p?.path) return badParams("missing path");
        await ctx.session.loadVrm(p.path);
        const id = `three-vrm-${++ctx.nextSessionId.value}`;
        return { ok: true, result: { session_id: id } };
      }
      case "set_camera": {
        await ctx.session.setCamera(params);
        return { ok: true, result: {} };
      }
      case "set_lighting": {
        await ctx.session.setLighting(params);
        return { ok: true, result: {} };
      }
      case "set_post_processing": {
        await ctx.session.setPostProcessing(params);
        return { ok: true, result: {} };
      }
      case "render": {
        const p = params as {
          width?: number;
          height?: number;
          output_path?: string;
          color_space?: string;
        };
        if (!p?.output_path) return badParams("missing output_path");
        const width = p.width ?? 1024;
        const height = p.height ?? 1024;
        const colorSpace = p.color_space ?? "Srgb";
        await ctx.session.render({
          width,
          height,
          color_space: colorSpace,
          output_path: p.output_path,
        });
        return {
          ok: true,
          result: {
            output_path: p.output_path,
            actual_color_space: colorSpace,
          },
        };
      }
      case "dispose": {
        await ctx.session.clearVrm();
        return { ok: true, result: {} };
      }
      case "step_physics": {
        await ctx.session.stepPhysics(params);
        return { ok: true, result: {} };
      }
      case "reset_physics": {
        await ctx.session.resetPhysics(params);
        return { ok: true, result: {} };
      }
      case "animate_root_transform": {
        await ctx.session.animateRootTransform(params);
        return { ok: true, result: {} };
      }
      case "dump_bone_positions": {
        const p = params as { session_id?: string; spring_index?: number };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpBonePositions(p);
        return { ok: true, result };
      }
      case "load_vrma": {
        const p = params as { vrma_path?: string };
        if (!p?.vrma_path) return badParams("missing vrma_path");
        const result = await ctx.session.loadVrma(p.vrma_path);
        return { ok: true, result };
      }
      case "apply_vrma_at_time": {
        const p = params as {
          session_id?: string;
          vrma_handle?: number;
          vrm_handle?: number;
          time_seconds?: number;
        };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.applyVrmaAtTime(p);
        return { ok: true, result };
      }
      case "dump_humanoid_pose": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpHumanoidPose();
        return { ok: true, result };
      }
      case "dump_expression_weights": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpExpressionWeights();
        return { ok: true, result };
      }
      case "dump_look_at_state": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpLookAtState();
        return { ok: true, result };
      }
      default: {
        const phase = PHASE_BY_RESERVED_METHOD[method];
        if (phase) {
          return {
            ok: false,
            error: {
              code: -32000,
              message: `${method}: not implemented in this adapter version`,
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
    }
  } catch (e) {
    const msg = (e as Error).message ?? String(e);
    if (method === "load_vrm") {
      return {
        ok: false,
        error: {
          code: -32001,
          message: "LoadFailed",
          data: { reason: msg },
        },
      };
    }
    if (method === "render") {
      return {
        ok: false,
        error: {
          code: -32002,
          message: "RenderFailed",
          data: { reason: msg },
        },
      };
    }
    return {
      ok: false,
      error: {
        code: -32603,
        message: `internal error: ${msg}`,
      },
    };
  }
}

function badParams(reason: string): DispatchFailure {
  return {
    ok: false,
    error: { code: -32602, message: `invalid params: ${reason}` },
  };
}

export function knownMethods(): string[] {
  return [
    "load_vrm",
    "set_camera",
    "set_lighting",
    "set_post_processing",
    "render",
    "dispose",
    "step_physics",
    "reset_physics",
    "animate_root_transform",
    "dump_bone_positions",
    "load_vrma",
    "apply_vrma_at_time",
    "dump_humanoid_pose",
    "dump_expression_weights",
    "dump_look_at_state",
    ...Object.keys(PHASE_BY_RESERVED_METHOD),
  ];
}
