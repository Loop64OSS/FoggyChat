<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { exit } from "@tauri-apps/plugin-process";
    import { navigate } from "svelte-routing";
    import { Toaster } from "@skeletonlabs/skeleton-svelte";
    import { toaster } from "../lib/toaster-svelte";
    import { Progress } from "@skeletonlabs/skeleton-svelte";
    import { onDestroy, onMount } from "svelte";
    import { listen } from "@tauri-apps/api/event";

    const appWebview = getCurrentWebviewWindow();
    let isWaiting = false;
    var serverAddress = "";
    let unlisten: () => void;

    function handleConnect() {
        isWaiting = true;
        invoke("request_connection", { address: serverAddress });
    }

    onMount(async () => {
        if (!unlisten) {
            unlisten = await listen("status", (event) => {
                console.log("[status]", event.payload);

                isWaiting = false;

                if (event.payload !== "ok") {
                    toaster.info({ title: event.payload });
                } else {
                    navigate("/messages");
                }
            });
        }
    });

    onDestroy(() => {
        if (unlisten) {
            unlisten();
        }
    });
    async function exitApp() {
        await exit(1);
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
        <h2 class="text-2xl font-semibold">Connecting to {serverAddress}</h2>
        <Progress value={null} />
    </div>
{/if}

<main class="flex h-screen justify-center mx-5">
    <button class="btn text-red-200 fixed right-0" on:click={exitApp}
        >Exit App</button
    >
    <div class="m-auto">
        <div class="flex items-center gap-4 mb-4">
            <p class="text-2xl">FoggyChat</p>
            <p class="text">DEV / 0.7</p>
        </div>
        <form on:submit|preventDefault={handleConnect}>
            <div class="input-group grid-cols-[1fr_auto]">
                <input
                    class="ig-input"
                    type="text"
                    placeholder="Server Address"
                    bind:value={serverAddress}
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
</main>
<Toaster {toaster} />
