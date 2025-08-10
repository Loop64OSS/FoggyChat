<!-- App.svelte -->
<script lang="ts">
    import { Router, Link, Route } from "svelte-routing";
    import MessagesPage from "./routes/MessagePage.svelte";
    import InitPage from "./routes/InitPage.svelte";
    import { invoke } from "@tauri-apps/api/core";
    export let url = "";
</script>

<Router {url}>
    <Route path="/"><InitPage /></Route>
    <Route path="/messages"><MessagesPage /></Route>
</Router>
