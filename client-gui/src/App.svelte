<!-- App.svelte -->
<script lang="ts">
    import { Router, Link, Route, navigate } from "svelte-routing";
    import MessagesPage from "./routes/MessagePage.svelte";
    import InitPage from "./routes/InitPage.svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { onDestroy, onMount, tick } from "svelte";
    import { listen } from "@tauri-apps/api/event";
    import { toaster } from "./lib/toaster-svelte";
    export let url = "";
    if (import.meta.env.MODE !== "development") {
        window.addEventListener("contextmenu", (e) => e.preventDefault());
    }
    let unlisten: () => void;
    onMount(async () => {
        if (!unlisten) {
            unlisten = await listen("status", async (event) => {
                console.log("[status]", event.payload);

                let status: string = event.payload as string;
                if (status === "USER::DISCONNECT") {
                    navigate("/");
                    tick();
                    toaster.info({ title: "Disconnected" });
                } else if (status.startsWith("E::")) {
                    status = status.replace("E::", "");
                    toaster.info({ title: status });
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

<Router {url}>
    <Route path="/"><InitPage /></Route>
    <Route path="/messages"><MessagesPage /></Route>
</Router>
