<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { navigate } from "svelte-routing";
    import { Toaster } from "@skeletonlabs/skeleton-svelte";
    import { toaster } from "../lib/toaster-svelte";
    import { Progress } from "@skeletonlabs/skeleton-svelte";
    import { onDestroy, onMount } from "svelte";
    import { listen } from "@tauri-apps/api/event";
    import { serverAddress } from "../lib/store.js";
    let isWaiting = false;
    let unlisten: () => void;
    let FpVerified = true;
    let FP = "";

    function handleConnect() {
        isWaiting = true;
        invoke("ui_command_request_connection", { address: $serverAddress });
    }

    onMount(async () => {
        if (!unlisten) {
            unlisten = await listen("status", async (event) => {
                console.log("[status]", event.payload);

                let status: string = event.payload as string;
                if (status === "OK::CON_ESTABLISHED" && FpVerified) {
                    isWaiting = false;
                    navigate("/messages");
                } else if (status.startsWith("USER::VERIFY_FP")) {
                    FP = status.split("::")[2];
                    FpVerified = false;
                    while (!FpVerified) {
                        await new Promise((resolve) => setTimeout(resolve, 10));
                    }
                    isWaiting = false;
                } else if (status.startsWith("E::")) {
                    status.replace("E::", "");
                    isWaiting = false;
                    toaster.info({ title: event.payload });
                }
            });
        }
    });

    onDestroy(() => {
        if (unlisten) {
            unlisten();
        }
    });
    function send_status(status: string) {
        invoke("ui_command_status", { input: status });
    }
</script>

{#if isWaiting}
    <!-- Backdrop -->
    <div class="fixed inset-0 bg-black/50 z-40 backdrop-blur-sm"></div>

    <!-- Modal -->
    <div
        class="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2
           z-50 body-background-color dark:body-background-color-dark text-inherit
           max-w-[640px] w-full rounded-xl p-6 shadow-xl space-y-4"
    >
        <h2 class="text-2xl font-semibold">Connecting to {$serverAddress}</h2>
        {#if !FpVerified}
            <h3
                class="card preset-filled-surface-200-800 px-2 py-1 overflow-auto"
            >
                {FP}
            </h3>
            <button
                class="btn preset-filled-primary-200-800"
                on:click={(_) => {
                    FpVerified = true;
                    send_status("USER::FP_MATCH");
                }}>Yes</button
            >
            <button
                class="btn preset-filled-secondary-200-800"
                on:click={(_) => {
                    send_status("USER::FP_MISMATCH");
                }}>No</button
            >
        {/if}
        <Progress value={null} />
    </div>
{/if}

<main class="flex flex-col justify-center mx-5">
    <div class="m-auto">
        <div class="flex items-center gap-4 mb-4">
            <p class="text-2xl">FoggyChat</p>
            <p class="text">DEV/0.9</p>
        </div>
        <form on:submit|preventDefault={handleConnect}>
            <div class="input-group grid-cols-[1fr_auto]">
                <input
                    class="ig-input"
                    type="text"
                    placeholder="Server Address"
                    bind:value={$serverAddress}
                />
                <button
                    class="ig-btn text-green-200 preset-filled-dark"
                    type="submit">Connect</button
                >
            </div>
        </form>
        <p>
            This is development version. This version should not be publicly
            available!
        </p>
    </div>
    <p>© Loop64 / FOG64</p>
</main>
<Toaster {toaster} />
