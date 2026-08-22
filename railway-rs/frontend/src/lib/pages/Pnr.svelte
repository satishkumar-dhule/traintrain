<script>
  import { api } from '$lib/api.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'

  let query = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let captcha = $state(null)
  let captchaText = $state('')

  const valid = $derived(/^\d{10}$/.test(query.trim()))

  async function lookup() {
    if (!valid) return
    phase = data ? 'refreshing' : 'loading'
    errorMsg = null
    let path = `/rail-api/pnr?pnr=${query.trim()}`
    if (captcha?.session_id) {
      path += `&captcha_session=${encodeURIComponent(captcha.session_id)}&captcha_text=${encodeURIComponent(captchaText.trim())}`
    }
    const res = await api(path)
    if (res.ok) {
      data = res.data
      phase = 'ok'
      captcha = null
      captchaText = ''
    } else if (res.status === 428 && res.body?.error === 'captcha_required') {
      captcha = res.body
      captchaText = ''
      phase = 'captcha'
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">PNR status</h1>
    <p class="text-sm text-muted-foreground">10-digit passenger name record. Upstream may require a captcha.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="pnr-no">PNR number</Label>
        <Input
          id="pnr-no"
          bind:value={query}
          placeholder="e.g. 1234567890"
          inputmode="numeric"
          maxlength={10}
          onkeydown={(e) => e.key === 'Enter' && lookup()}
        />
        {#if query && !valid}
          <p class="text-xs text-muted-foreground">PNR must be exactly 10 digits.</p>
        {/if}
      </div>
      <button
        type="button"
        class="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        onclick={lookup}
        disabled={!valid || phase === 'loading' || phase === 'refreshing'}
      >
        Check status
      </button>
    </Card.Content>
  </Card.Root>

  {#if phase === 'captcha' && captcha}
    <Card.Root>
      <Card.Header>
        <Card.Title>Captcha required</Card.Title>
        <Card.Description>{captcha.message ?? 'Enter the characters shown to continue.'}</Card.Description>
      </Card.Header>
      <Card.Content class="grid gap-4">
        {#if captcha.image}
          <img
            src={captcha.image.startsWith('data:') ? captcha.image : `data:image/jpeg;base64,${captcha.image}`}
            alt="PNR captcha challenge"
            class="w-full max-w-[240px] rounded-md border"
          />
        {:else}
          <Alert.Root variant="destructive">
            <Alert.Title>No captcha image supplied</Alert.Title>
            <Alert.Description>Upstream did not return an image. Try again.</Alert.Description>
          </Alert.Root>
        {/if}
        <div class="flex flex-wrap items-end gap-3">
          <div class="grid flex-1 gap-2 sm:max-w-56">
            <Label for="cap-text">Captcha text</Label>
            <Input id="cap-text" bind:value={captchaText} placeholder="5 characters" />
          </div>
          <button
            type="button"
            class="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            onclick={lookup}
            disabled={!captchaText.trim()}
          >
            Submit
          </button>
        </div>
      </Card.Content>
    </Card.Root>
  {:else if phase === 'loading'}
    <div class="grid gap-2" aria-busy="true">
      {#each [0, 1, 2] as i (i)}
        <Skeleton class="h-12 w-full" />
      {/each}
    </div>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load PNR</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if data}
    <div class="grid gap-4">
      <Card.Root>
        <Card.Header class="flex-row items-center justify-between space-y-0">
          <div class="grid gap-1">
            <Card.Title>
              <span class="font-mono">{data.train_number ?? '—'}</span> · {data.train_name ?? ''}
            </Card.Title>
            <Card.Description>
              {fmt(data.from?.code)} → {fmt(data.to?.code)} · journey {fmt(data.journey_date)}
            </Card.Description>
          </div>
          {#if data.data_source}<Badge variant="outline">{data.data_source}</Badge>{/if}
        </Card.Header>
        <Card.Content>
          <Table.Root>
            <Table.Header>
              <Table.Row>
                <Table.Head>#</Table.Head>
                <Table.Head>Booking status</Table.Head>
                <Table.Head>Coach</Table.Head>
                <Table.Head>Berth</Table.Head>
                <Table.Head>Current</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.passengers ?? [] as p, i (i)}
                <Table.Row>
                  <Table.Cell class="text-muted-foreground">{i + 1}</Table.Cell>
                  <Table.Cell class="font-medium">{fmt(p.booking_status)}</Table.Cell>
                  <Table.Cell class="font-mono">{fmt(p.coach)}</Table.Cell>
                  <Table.Cell class="font-mono">{fmt(p.berth)}</Table.Cell>
                  <Table.Cell class="font-mono">{fmt(p.current_status)}</Table.Cell>
                </Table.Row>
              {:else}
                <Table.Row>
                  <Table.Cell colspan={5} class="text-muted-foreground">No passengers returned.</Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </Card.Content>
      </Card.Root>
    </div>
  {/if}
</section>
