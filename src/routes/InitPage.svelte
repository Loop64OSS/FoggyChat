<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { navigate } from "svelte-routing";
    import { Toaster } from "@skeletonlabs/skeleton-svelte";
    import { toaster } from "../lib/toaster-svelte";
    import { Progress } from "@skeletonlabs/skeleton-svelte";
    import { onDestroy, onMount } from "svelte";
    import { listen } from "@tauri-apps/api/event";
    import { serverAddress } from "../lib/store.js";
    import { load } from "@tauri-apps/plugin-store";

    import {
        isPermissionGranted,
        requestPermission,
        sendNotification,
    } from "@tauri-apps/plugin-notification";
    import { Check, GlobeLock, LogIn, X } from "@lucide/svelte";
    let isWaiting: boolean = false;
    let unlisten: () => void;
    let FpVerified: boolean = true;
    let FP: string;
    let lastConnectionAddress: string;

    function handleConnect() {
        isWaiting = true;
        invoke("ui_command_request_connection", { address: $serverAddress });
    }
    function sendStatus(status: string) {
        invoke("ui_command_status", { input: status });
    }
    async function requestPermissions() {
        // Do you have permission to send a notification?
        let permissionGranted = await isPermissionGranted();

        // If not we need to request it
        if (!permissionGranted) {
            const permission = await requestPermission();
            permissionGranted = permission === "granted";
        }
    }
    async function fillConnectionAddress() {
        $serverAddress = lastConnectionAddress;
    }
    onMount(async () => {
        requestPermissions();

        const store = await load("store.json", {
            autoSave: false,
            defaults: {},
        });
        const lastConnection = await store.get<{ address: string }>(
            "last-connection"
        );
        lastConnectionAddress = lastConnection?.address ?? "";

        //Status listening from backend
        if (!unlisten) {
            unlisten = await listen("status", async (event) => {
                console.log("[status]", event.payload);
                let payload: any = event.payload;

                if (payload.msg === "con_established" && FpVerified) {
                    isWaiting = false;
                    await store.set("last-connection", {
                        address: $serverAddress,
                    });
                    await store.save();

                    navigate("/messages");
                } else if (payload.status == "user::verify_fp") {
                    FP = payload.msg;
                    FpVerified = false;
                    while (!FpVerified) {
                        await new Promise((resolve) => setTimeout(resolve, 10));
                    }
                    isWaiting = false;
                } else if (payload.status == "error") {
                    isWaiting = false;
                }
            });
        }
    });

    onDestroy(() => {
        if (unlisten) {
            unlisten();
        }
    });
</script>

{#if isWaiting}
    <!-- Backdrop -->
    <div class="fixed inset-0 bg-black/50 z-40 backdrop-blur-sm"></div>

    <!-- Modal -->
    <div
        class="fixed top-1/2 left-1/2
           z-50 body-background-color dark:body-background-color-dark text-inherit
           max-w-[640px] w-full rounded-container p-6 shadow-xl space-y-2 animate-fadeIn"
    >
        <header class="mb-4 flex flex-col sm:flex-row justify-between">
            <h2 class="text-2xl font-semibold text-clip mr-4 break-all">
                Connecting to {$serverAddress}
            </h2>
            <button
                class="btn preset-filled-error-200-800 hover:scale-105 active:scale-95 transition-transform h-fit"
                on:click={(_) => {
                    isWaiting = false;
                    FpVerified = true;
                    sendStatus("USER::DISCONNECT");
                }}><X size={18} /> Cancel</button
            >
        </header>
        {#if FpVerified}
            <div class="card preset-outlined-surface-200-800 p-2">
                <Progress value={null} />
            </div>
        {/if}
        {#if !FpVerified}
            <p>
                Please verify the fingerprint to ensure the key wasn’t spoofed:
            </p>
            <div class="flex gap-1 items-center">
                <p>BLAKE3</p>
                <p
                    class="card preset-outlined-surface-200-800 px-2 py-1 overflow-auto"
                >
                    {FP}
                </p>
            </div>
            <footer class="pt-4 flex justify-end gap-3">
                <nav class="btn-group preset-outlined-surface-200-800 p-2">
                    <button
                        class="btn preset-filled-primary-500 hover:scale-105 active:scale-95 transition-transform"
                        on:click={(_) => {
                            isWaiting = false;
                            FpVerified = true;
                            sendStatus("USER::FP_MATCH");
                        }}><Check size={18} /> Yes</button
                    >
                    <button
                        class="btn preset-outlined-primary-500 hover:scale-105 active:scale-95 transition-transform"
                        on:click={(_) => {
                            isWaiting = false;
                            FpVerified = true;
                            sendStatus("USER::FP_MISMATCH");
                        }}><X size={18} /> No</button
                    >
                </nav>
            </footer>
        {/if}
    </div>
{/if}

<main class="flex flex-col justify-center h-full max-w-dvw">
    <div class="m-auto max-w-dvw">
        <div class=" mx-2">
            <div class="flex items-center gap-4 mb-4">
                <p class="text-2xl">FoggyChat</p>
                <p class="text">DEV/2.2</p>
            </div>
            <form on:submit|preventDefault={handleConnect}>
                <div class="flex flex-col">
                    <div class="grid grid-cols-[1fr_auto] gap-2">
                        <input
                            class="input rounded-base focus:ring-2 focus:ring-primary-500"
                            id="msginput"
                            type="text"
                            placeholder="Server address"
                            bind:value={$serverAddress}
                            required
                        />

                        <button
                            class="btn px-3 preset-filled-surface-200-800 rounded-base hover:scale-105 active:scale-95 transition-transform"
                            type="submit"
                        >
                            Connect
                        </button>
                    </div>
                    {#if lastConnectionAddress}
                        <button
                            class="btn mt-2 preset-outlined-surface-200-800 rounded-base hover:scale-105 active:scale-95 max-w-screen transition-transform break-all whitespace-normal"
                            on:click={fillConnectionAddress}
                            type="button"
                            >Click to fill: {lastConnectionAddress}</button
                        >
                    {/if}
                </div>
            </form>
        </div>
    </div>
    <div class="m-2">
        <p>This is a development version. It should not be shared.</p>
        <p>© Loop64™ / FOG Privacy Toolkit</p>
    </div>
</main>

<style>
    @keyframes fadeIn {
        from {
            opacity: 0;
            transform: translate(-50%, -40%);
        }
        to {
            opacity: 1;
            transform: translate(-50%, -50%);
        }
    }
    .animate-fadeIn {
        animation: fadeIn 0.3s ease forwards;
    }
</style>
