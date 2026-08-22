<script>
  import { api } from "$lib/api";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
  } from "$lib/components/ui/card/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
  } from "$lib/components/ui/table/index.js";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import {
    Alert,
    AlertDescription,
    AlertTitle,
  } from "$lib/components/ui/alert/index.js";

  const DAY_LETTERS = ["M", "T", "W", "T", "F", "S", "S"];

  let src = $state("");
  let dst = $state("");
  let loading = $state(false);
  let attempted = $state(false);
  let result = $state(null);
  let errorState = $state(null);
  let searchedSrc = $state("");
  let searchedDst = $state("");

  let canSearch = $derived(src.trim().length > 0 && dst.trim().length > 0);
  let trains = $derived(Array.isArray(result?.trains) ? result.trains : []);

  function onInputSrc(e) {
    src = e.currentTarget.value.toUpperCase();
  }

  function onInputDst(e) {
    dst = e.currentTarget.value.toUpperCase();
  }

  async function search() {
    attempted = true;
    if (!canSearch || loading) return;
    const qSrc = encodeURIComponent(src.trim());
    const qDst = encodeURIComponent(dst.trim());
    loading = true;
    errorState = null;
    result = null;
    try {
      const res = await api(`/rail-api/ntes/trains-between?src=${qSrc}&dst=${qDst}`);
      if (res.ok) {
        result = res.data;
        searchedSrc = src.trim();
        searchedDst = dst.trim();
      } else {
        errorState = { message: res.error ?? "Unknown error", status: res.status };
      }
    } finally {
      loading = false;
    }
  }

  function onKeydown(e) {
    if (e.key === "Enter") search();
  }
</script>

<div class="mx-auto flex w-full max-w-3xl flex-col gap-4 p-4">
  <Card>
    <CardHeader>
      <CardTitle>Trains between stations</CardTitle>
      <CardDescription>Enter station codes, e.g. NDLS to DLI</CardDescription>
    </CardHeader>
    <CardContent class="flex flex-col gap-2">
      <div class="flex items-end gap-2">
        <label class="flex w-full min-w-0 flex-col gap-1.5">
          <span class="text-sm font-medium">From</span>
          <Input
            bind:value={src}
            oninput={onInputSrc}
            onkeydown={onKeydown}
            placeholder="NDLS"
            maxlength="10"
            disabled={loading}
            aria-label="From station code"
          />
        </label>
        <label class="flex w-full min-w-0 flex-col gap-1.5">
          <span class="text-sm font-medium">To</span>
          <Input
            bind:value={dst}
            oninput={onInputDst}
            onkeydown={onKeydown}
            placeholder="DLI"
            maxlength="10"
            disabled={loading}
            aria-label="To station code"
          />
        </label>
        <Button onclick={search} disabled={loading || !canSearch}>Search</Button>
      </div>
      {#if attempted && !canSearch}
        <p class="text-xs text-muted-foreground">
          Enter both a From and a To station code to search.
        </p>
      {/if}
    </CardContent>
  </Card>

  {#if loading}
    <Card>
      <CardContent class="space-y-3 pt-6">
        {#each [0, 1, 2] as row (row)}
          <div class="flex items-center gap-4">
            <Skeleton class="h-5 w-16" />
            <Skeleton class="h-4 w-48" />
            <Skeleton class="h-4 w-14" />
            <Skeleton class="h-4 w-14" />
            <div class="ml-auto flex gap-1">
              {#each [0, 1, 2, 3, 4, 5, 6] as d (d)}
                <Skeleton class="h-5 w-5 rounded-full" />
              {/each}
            </div>
          </div>
        {/each}
      </CardContent>
    </Card>
  {:else if errorState}
    <Alert destructive>
      <AlertTitle>Could not load trains</AlertTitle>
      <AlertDescription>
        {errorState.message}{errorState.status ? ` (status ${errorState.status})` : ""}
      </AlertDescription>
    </Alert>
  {:else if result}
    <Card>
      <CardHeader>
        <CardTitle>Trains {searchedSrc} → {searchedDst}</CardTitle>
        <CardDescription>{trains.length} trains found</CardDescription>
      </CardHeader>
      <CardContent>
        {#if trains.length === 0}
          <p class="text-sm text-muted-foreground">
            No trains found between {searchedSrc} and {searchedDst}.
          </p>
        {:else}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Train</TableHead>
                <TableHead>Departs</TableHead>
                <TableHead>Arrives</TableHead>
                <TableHead>Runs on</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {#each trains as t, i (t.number ?? i)}
                <TableRow>
                  <TableCell>
                    <span class="flex items-center gap-2">
                      <Badge variant="secondary">{t.number}</Badge>
                      <span>{t.name}</span>
                    </span>
                  </TableCell>
                  <TableCell>{t.departure_time}</TableCell>
                  <TableCell>{t.arrival_time}</TableCell>
                  <TableCell>
                    <span class="flex flex-wrap items-center gap-1">
                      {#each t.runs_on ?? [] as active, di (di)}
                        <Badge
                          variant={active ? "default" : "outline"}
                          class="flex h-5 w-5 items-center justify-center px-1 text-[10px]"
                        >
                          {DAY_LETTERS[di] ?? ""}
                        </Badge>
                      {/each}
                    </span>
                  </TableCell>
                </TableRow>
              {/each}
            </TableBody>
          </Table>
        {/if}
      </CardContent>
    </Card>
  {/if}
</div>
