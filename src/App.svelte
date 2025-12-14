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

                let payload: any = event.payload;
                if (payload.status == "user" && payload.msg == "disconnect") {
                    navigate("/");
                    tick();
                    toaster.info({ title: "Disconnected" });
                } else if (payload.status == "error") {
                    toaster.info({ title: payload.msg });
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
