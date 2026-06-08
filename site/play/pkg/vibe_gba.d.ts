/* tslint:disable */
/* eslint-disable */

export class WebGba {
    free(): void;
    [Symbol.dispose](): void;
    debug_summary(): string;
    height(): number;
    constructor(rom: Uint8Array);
    run_frame(): Uint8Array;
    set_button(button: number, pressed: boolean): void;
    width(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_webgba_free: (a: number, b: number) => void;
    readonly webgba_debug_summary: (a: number) => [number, number];
    readonly webgba_height: (a: number) => number;
    readonly webgba_new: (a: number, b: number) => number;
    readonly webgba_run_frame: (a: number) => [number, number];
    readonly webgba_set_button: (a: number, b: number, c: number) => void;
    readonly webgba_width: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
