<script>
  // IntentForm.svelte — rich text form shown when the bot is not sure, prefilled with whatever was collected.
  // Matches app styling (Tailwind, rounded-md inputs, rounded-lg card) and keeps REQUIRED_FIELDS in sync with gate.js
  import SearchIcon from 'lucide-svelte/icons/search';
  import ClipboardCheck from 'lucide-svelte/icons/clipboard-check';

  // Keep in sync with gate.js INTENTS needs* — see gate.js: INTENTS & REQUIRED slot logic
  export const REQUIRED_FIELDS = {
    live_status: ['train'],
    average_delay: ['train'],
    train_schedule: ['train'],
    trains_between: ['src', 'dst'],
    station_board: ['station'],
    seat_availability: ['src', 'dst'],
    chart_status: ['train'],
    pnr_status: ['pnr']
  };

  export const OPTIONAL_FIELDS = {
    seat_availability: ['date'],
    chart_status: ['date'],
    trains_between: [],
    live_status: [],
    average_delay: [],
    train_schedule: [],
    station_board: [],
    pnr_status: []
  };

  const FIELD_META = {
    train: { label: 'Train number', placeholder: 'e.g. 12951', hint: '5-digit number', required: true, type: 'text', inputMode: 'numeric', maxLength: 5, pattern: '\\d{5}' },
    pnr: { label: 'PNR number', placeholder: 'e.g. 1234567890', hint: '10-digit PNR from ticket', required: true, type: 'text', inputMode: 'numeric', maxLength: 10, pattern: '\\d{10}' },
    src: { label: 'From station', placeholder: 'e.g. SC or Secunderabad', hint: 'Station code or name', required: true, type: 'text' },
    dst: { label: 'To station', placeholder: 'e.g. PUNE or Pune Jn', hint: 'Station code or name', required: true, type: 'text' },
    station: { label: 'Station', placeholder: 'e.g. PUNE or Pune Jn', hint: 'Station code or name', required: true, type: 'text' },
    date: { label: 'Date', placeholder: '', hint: 'Optional · within 120 days', required: false, type: 'date' }
  };

  const INTENT_OPTIONS = [
    { id: 'live_status', label: 'Live status' },
    { id: 'trains_between', label: 'Trains between' },
    { id: 'seat_availability', label: 'Seat availability' },
    { id: 'station_board', label: 'Station board' },
    { id: 'train_schedule', label: 'Train schedule' },
    { id: 'average_delay', label: 'Average delay' },
    { id: 'chart_status', label: 'Chart status' },
    { id: 'pnr_status', label: 'PNR status' }
  ];

  const INTENT_LABEL_MAP = Object.fromEntries(INTENT_OPTIONS.map(o => [o.id, o.label]));

  export let form = {
    intentId: null,
    intentLabel: '',
    confidence: 0,
    collected: {},
    missing: [],
    fields: null,
    candidates: null
  };
  export let onSubmit = (payload) => {};
  export let onCancel = () => {};

  // internal intent state — allows picker to swap fields dynamically
  let selectedIntentId = form.intentId ?? null;

  $: if (form.intentId !== undefined && form.intentId !== selectedIntentId && form.intentId !== null) {
    // keep in sync if parent updates intentId after mount (but don't overwrite user's picker choice when null)
    // Only sync when form.intentId is truthy and differs from internal
    // If form.intentId is null, keep current selection
    if (form.intentId) selectedIntentId = form.intentId;
  }

  // values holds all possible keys; prefilled immediately from form.collected and form.fields
  function buildInitialValues(f) {
    const v = { train: '', pnr: '', src: '', dst: '', station: '', date: '' };
    const c = f?.collected ?? {};
    for (const k of Object.keys(v)) {
      if (c[k] != null) v[k] = String(c[k]);
      else if (c[k.toUpperCase()] != null) v[k] = String(c[k.toUpperCase()]);
    }
    // also pull from fields[].value if fields supplied
    if (Array.isArray(f?.fields)) {
      for (const fld of f.fields) {
        if (fld?.key && fld?.value != null && String(fld.value).length) {
          v[fld.key] = String(fld.value);
        }
        // some callers may use name instead of key
        if (fld?.name && fld?.value != null && String(fld.value).length) {
          v[fld.name] = String(fld.value);
        }
      }
    }
    // also support flat props like form.train etc (defensive)
    for (const k of Object.keys(v)) if (f?.[k] != null && !v[k]) v[k] = String(f[k]);
    return v;
  }

  let values = buildInitialValues(form);
  $: {
    // rebuild when form changes — keep user's edits unless they were empty?
    // Use a shallow merge: preserve non-empty user values if new collected is empty
    const fresh = buildInitialValues(form);
    for (const k of Object.keys(values)) {
      if (fresh[k] && fresh[k] !== values[k]) {
        // if fresh has a prefilled value that differs, adopt it only if current is empty or equals old collected
        if (!values[k] || values[k] === '' ) values[k] = fresh[k];
        else {
          // if fresh is non-empty and current is empty, adopt; if fresh overwrites collected, adopt
          // Prefer fresh if it came from collected badge
          if (fresh[k] && values[k] !== fresh[k]) {
            // Only overwrite if the old value was exactly the previous collected? Simpler: allow fresh to fill missing
            // Keep user typing intact when they already typed something
          }
        }
      }
    }
    // ensure any new collected that we missed gets filled when empty
    for (const k of Object.keys(fresh)) if (fresh[k] && !values[k]) values[k] = fresh[k];
  }

  let touched = {};
  let blurred = {};

  const todayPlus120 = (() => {
    const d = new Date(Date.now() + 120 * 86400000);
    return d.toISOString().slice(0, 10);
  })();

  $: candidatesList = (() => {
    if (Array.isArray(form.candidates) && form.candidates.length) {
      // normalize candidates to {id,label}
      return form.candidates.map(c => {
        if (typeof c === 'string') return { id: c, label: INTENT_LABEL_MAP[c] ?? c };
        return { id: c.id ?? c.value ?? c.intentId, label: c.label ?? INTENT_LABEL_MAP[c.id] ?? c.id };
      }).filter(c => c.id);
    }
    return INTENT_OPTIONS;
  })();

  $: showPicker = candidatesList.length > 0 && (form.intentId == null || (typeof form.confidence === 'number' && form.confidence < 0.35));

  $: effectiveIntentId = selectedIntentId ?? form.intentId ?? null;
  $: effectiveLabel = (() => {
    if (effectiveIntentId && INTENT_LABEL_MAP[effectiveIntentId]) return INTENT_LABEL_MAP[effectiveIntentId];
    if (form.intentLabel) return form.intentLabel;
    if (effectiveIntentId) return effectiveIntentId;
    return '';
  })();

  $: title = effectiveIntentId ? `Check ${effectiveLabel}` : 'What would you like to check?';

  $: subtitle = (() => {
    const conf = Number(form.confidence);
    const miss = Array.isArray(form.missing) ? form.missing : [];
    if (!Number.isNaN(conf) && conf >= 0.30 && conf <= 0.45) return "I wasn't quite sure — confirm the details below.";
    if (miss.length > 0) return "Just need a bit more info.";
    if (effectiveIntentId && conf < 0.45 && conf > 0) return "I wasn't quite sure — confirm the details below.";
    return "";
  })();

  // derive fields to render
  $: derivedFields = (() => {
    if (Array.isArray(form.fields) && form.fields.length) {
      // normalize form.fields entries
      return form.fields.map(fld => {
        const key = fld.key ?? fld.name ?? fld.id;
        const meta = FIELD_META[key] ?? {};
        return {
          key,
          label: fld.label ?? meta.label ?? key,
          required: fld.required ?? meta.required ?? false,
          placeholder: fld.placeholder ?? meta.placeholder ?? '',
          hint: fld.hint ?? meta.hint ?? '',
          type: fld.type ?? meta.type ?? 'text',
          inputMode: fld.inputMode ?? meta.inputMode,
          maxLength: fld.maxLength ?? meta.maxLength,
          pattern: fld.pattern ?? meta.pattern,
          value: fld.value ?? values[key] ?? ''
        };
      });
    }
    // derive from selected intent
    const id = effectiveIntentId;
    if (!id || !REQUIRED_FIELDS[id]) {
      // no intent yet — show nothing until picker chooses; but if showPicker false, show empty
      return [];
    }
    const req = REQUIRED_FIELDS[id] ?? [];
    const opt = OPTIONAL_FIELDS[id] ?? [];
    const keys = [...req, ...opt];
    return keys.map(k => {
      const meta = FIELD_META[k];
      return {
        key: k,
        label: meta.label,
        required: req.includes(k),
        placeholder: meta.placeholder,
        hint: meta.hint,
        type: meta.type,
        inputMode: meta.inputMode,
        maxLength: meta.maxLength,
        pattern: meta.pattern,
        value: values[k] ?? ''
      };
    });
  })();

  // for picker changes: when intent switches, keep values but recompute fields
  function selectIntent(id) {
    selectedIntentId = id;
    // reset touched for new fields to avoid stale errors
    // keep values as-is
  }

  function fieldError(key, val) {
    const meta = FIELD_META[key];
    const required = (() => {
      const f = derivedFields.find(x => x.key === key);
      if (f) return !!f.required;
      // fallback to REQUIRED_FIELDS
      if (effectiveIntentId && REQUIRED_FIELDS[effectiveIntentId]?.includes(key)) return true;
      return !!meta?.required;
    })();
    const v = String(val ?? '').trim();
    if (required && !v) return 'Required';
    if (key === 'train' && v) {
      if (!/^\d{5}$/.test(v)) return 'Enter 5 digits';
    }
    if (key === 'pnr' && v) {
      if (!/^\d{10}$/.test(v)) return 'Enter 10 digits';
    }
    return '';
  }

  $: errors = (() => {
    const e = {};
    for (const f of derivedFields) {
      e[f.key] = fieldError(f.key, values[f.key]);
    }
    return e;
  })();

  $: requiredKeys = derivedFields.filter(f => f.required).map(f => f.key);
  $: isFormValid = requiredKeys.length > 0 ? requiredKeys.every(k => !fieldError(k, values[k])) : false;

  // exported helper for tests
  export function isValid() {
    return isFormValid;
  }

  $: firstMissingKey = (() => {
    const miss = Array.isArray(form.missing) ? form.missing : [];
    if (miss.length) {
      // find first missing that is in derivedFields
      for (const k of miss) if (derivedFields.some(f => f.key === k)) return k;
      return miss[0];
    }
    // fallback: first required empty
    for (const f of derivedFields) if (f.required && !String(values[f.key] ?? '').trim()) return f.key;
    return derivedFields[0]?.key ?? null;
  })();

  function handleBlur(key) {
    touched[key] = true;
    blurred[key] = true;
  }

  function handleInput(key, e) {
    values[key] = e.target.value;
    // Svelte reactivity for object: reassign
    values = { ...values };
  }

  function handleSubmit() {
    if (!isFormValid) return;
    const payloadValues = {
      train: String(values.train ?? '').trim() || undefined,
      pnr: String(values.pnr ?? '').trim() || undefined,
      src: String(values.src ?? '').trim() || undefined,
      dst: String(values.dst ?? '').trim() || undefined,
      station: String(values.station ?? '').trim() || undefined,
      date: String(values.date ?? '').trim() || undefined
    };
    // Remove undefined to keep payload clean but keep keys for spec compliance (spec shows all 5)
    // Keep undefined as omitted? Spec says values:{train, src, dst, station, date} — send all
    onSubmit({ intentId: effectiveIntentId, values: payloadValues });
  }

  function handleCancel() {
    onCancel();
  }

  $: submitLabel = effectiveLabel ? `Check ${effectiveLabel}` : 'Search';
</script>

<div class="max-w-full rounded-lg border bg-card p-4 flex flex-col gap-3 overflow-hidden" role="group" aria-label="Intent form">
  <div class="flex items-start gap-2.5">
    <div class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary">
      {#if effectiveIntentId === 'chart_status'}
        <ClipboardCheck class="size-4" />
      {:else}
        <SearchIcon class="size-4" />
      {/if}
    </div>
    <div class="min-w-0 flex-1">
      <h3 class="text-sm font-semibold leading-none tracking-tight">{title}</h3>
      {#if subtitle}
        <p class="mt-1 text-sm text-muted-foreground">{subtitle}</p>
      {/if}
    </div>
  </div>

  <form class="flex flex-col gap-3 max-w-full min-w-0" on:submit|preventDefault={handleSubmit} novalidate>
    {#if showPicker}
      <div class="grid gap-2" role="group" aria-label="Choose what to check">
        <p class="text-xs font-medium text-muted-foreground">Choose what to check</p>
        <div class="grid grid-cols-2 gap-2 sm:grid-cols-2 max-w-full">
          {#each candidatesList as c (c.id)}
            <button
              type="button"
              role="button"
              aria-pressed={effectiveIntentId === c.id}
              class="rounded-full border px-3 py-1.5 text-xs font-medium transition-colors min-h-8 flex items-center justify-center text-center max-w-full truncate {effectiveIntentId === c.id ? 'bg-primary text-primary-foreground border-primary shadow-sm' : 'bg-muted/60 text-muted-foreground hover:bg-muted hover:text-foreground border-border'}"
              on:click={() => selectIntent(c.id)}
            >{c.label}</button>
          {/each}
        </div>
      </div>
    {/if}

    {#if derivedFields.length > 0}
      <div class="grid gap-3 max-w-full min-w-0">
        {#each derivedFields as field (field.key)}
          {@const val = values[field.key] ?? ''}
          {@const isCollected = (() => {
            const col = form.collected ?? {};
            return col[field.key] != null && String(col[field.key]).trim() !== '' && String(col[field.key]).trim() === String(val).trim() && String(val).trim() !== '';
          })()}
          {@const err = errors[field.key] ?? ''}
          {@const showErr = (touched[field.key] || blurred[field.key]) && err}
          {@const isMissing = form.missing?.includes(field.key)}
          {@const isFirstMissing = field.key === firstMissingKey}
          <div class="grid gap-1.5 min-w-0 max-w-full">
            <div class="flex items-center gap-2 min-w-0">
              <label for="intent-field-{field.key}" class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 flex items-center gap-1 min-w-0">
                <span class="truncate">{field.label}</span>
                {#if field.required}
                  <span class="text-destructive" aria-hidden="true">*</span>
                {:else}
                  <span class="text-xs font-normal text-muted-foreground">(optional)</span>
                {/if}
              </label>
              {#if isCollected}
                <span class="ml-auto shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground border">prefilled from your message</span>
              {/if}
            </div>
            <input
              id="intent-field-{field.key}"
              name={field.key}
              type={field.type}
              placeholder={field.placeholder}
              value={val}
              on:input={(e) => handleInput(field.key, e)}
              on:blur={() => handleBlur(field.key)}
              aria-invalid={showErr ? 'true' : 'false'}
              aria-describedby={field.hint ? `hint-${field.key}` : undefined}
              autofocus={isFirstMissing ? true : undefined}
              inputmode={field.inputMode}
              maxlength={field.maxLength}
              pattern={field.pattern}
              max={field.type === 'date' ? todayPlus120 : undefined}
              class="flex h-9 w-full max-w-full min-w-0 rounded-md border bg-background px-3 py-1 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:border-ring disabled:opacity-50 transition-colors {showErr ? 'border-destructive ring-1 ring-destructive/30' : isCollected ? 'bg-muted/40 border-input' : 'border-input'} {isFirstMissing && isMissing ? 'ring-1 ring-ring' : ''}"
            />
            {#if field.hint}
              <p id="hint-{field.key}" class="text-xs text-muted-foreground leading-tight">{field.hint}</p>
            {/if}
            {#if showErr}
              <p class="text-xs text-destructive" role="alert">{err}</p>
            {/if}
          </div>
        {/each}
        <div class="rounded-md bg-muted/40 border border-dashed px-3 py-2 text-xs text-muted-foreground leading-relaxed max-w-full">
          <p class="font-medium text-foreground text-xs mb-1">Examples</p>
          <ul class="list-disc pl-4 space-y-0.5 max-w-full">
            <li class="break-words [overflow-wrap:anywhere]">“live status of 12951”</li>
            <li class="break-words [overflow-wrap:anywhere]">“trains from SC to PUNE” or “SC → PUNE”</li>
            <li class="break-words [overflow-wrap:anywhere]">“seat availability SC to PUNE tomorrow”</li>
            <li class="break-words [overflow-wrap:anywhere]">“station board PUNE”</li>
            <li class="break-words [overflow-wrap:anywhere]">“pnr status 1234567890”</li>
          </ul>
        </div>
      </div>
    {:else if !showPicker}
      <p class="text-sm text-muted-foreground">Select an intent above to fill the details.</p>
    {/if}

    <div class="flex items-center justify-end gap-2 pt-1 max-w-full">
      <button
        type="button"
        class="inline-flex h-9 items-center justify-center rounded-md border border-transparent bg-transparent px-3 text-sm font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        on:click={handleCancel}
      >Cancel</button>
      <button
        type="submit"
        disabled={!isFormValid}
        class="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-sm hover:bg-primary/90 disabled:opacity-50 disabled:pointer-events-none transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >{submitLabel}</button>
    </div>
  </form>
</div>

<style>
  /* ensure no horizontal overflow inside card */
  :global(.intent-form-input) { max-width: 100%; }
</style>
