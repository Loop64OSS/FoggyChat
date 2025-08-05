import type { Machine, MachineSchema, Service } from "@zag-js/core";
export declare function useMachine<T extends MachineSchema>(machine: Machine<T>, userProps: Partial<T["props"]> | (() => Partial<T["props"]>)): Service<T>;
