<script setup lang="ts">
import { reactive, ref, nextTick } from 'vue'

const WEBHOOK =
  'https://digilab.overheid.nl/chat/hooks/khcsah5zg3gy8notbfy5baoxwh'

type Status = 'idle' | 'submitting' | 'success' | 'error'

const status = ref<Status>('idle')

const form = reactive({
  honeypot: '',
  bijdragen: 'Ja',
  opDeHoogte: true,
  email: '',
  naam: '',
  organisatie: '',
  functie: '',
})

const errors = reactive<{ email: string; naam: string }>({
  email: '',
  naam: '',
})

const emailInput = ref<HTMLInputElement | null>(null)
const naamInput = ref<HTMLInputElement | null>(null)
const successHeading = ref<HTMLElement | null>(null)
const errorHeading = ref<HTMLElement | null>(null)
const firstFieldFocus = ref<HTMLInputElement | null>(null)

function validate(): boolean {
  errors.email = ''
  errors.naam = ''
  if (!form.email.trim()) {
    errors.email = 'Vul je e-mailadres in.'
  } else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim())) {
    errors.email = 'Vul een geldig e-mailadres in.'
  }
  if (!form.naam.trim()) {
    errors.naam = 'Vul je volledige naam in.'
  }
  return !errors.email && !errors.naam
}

async function onSubmit() {
  // Honeypot: a real user never fills this.
  if (form.honeypot) return

  if (!validate()) {
    await nextTick()
    if (errors.email) emailInput.value?.focus()
    else naamInput.value?.focus()
    return
  }

  status.value = 'submitting'

  const text =
    '#### Nieuwe aanmelding: RegelRecht\n' +
    '| Veld | Waarde |\n' +
    '|:-----|:-------|\n' +
    `| **Naam** | ${form.naam} |\n` +
    `| **E-mail** | ${form.email} |\n` +
    `| **Organisatie** | ${form.organisatie || '-'} |\n` +
    `| **Functie** | ${form.functie || '-'} |\n` +
    `| **Bijdragen aan validatie** | ${form.bijdragen} |\n` +
    `| **Op de hoogte blijven** | ${form.opDeHoogte ? 'Ja' : 'Nee'} |`

  try {
    await fetch(WEBHOOK, {
      method: 'POST',
      mode: 'no-cors',
      body: JSON.stringify({ text }),
    })
    // no-cors yields an opaque response: a resolved promise means the
    // request left the browser, not that the server accepted it. We frame
    // the success copy accordingly.
    status.value = 'success'
    await nextTick()
    successHeading.value?.focus()
  } catch {
    status.value = 'error'
    await nextTick()
    errorHeading.value?.focus()
  }
}

async function reset() {
  status.value = 'idle'
  form.email = ''
  form.naam = ''
  form.organisatie = ''
  form.functie = ''
  form.bijdragen = 'Ja'
  form.opDeHoogte = true
  errors.email = ''
  errors.naam = ''
  await nextTick()
  firstFieldFocus.value?.focus()
}
</script>

<template>
  <div>
    <form
      v-if="status === 'idle' || status === 'submitting'"
      class="rr-form"
      novalidate
      :aria-busy="status === 'submitting'"
      @submit.prevent="onSubmit"
    >
      <!-- Honeypot, off-screen and out of the a11y tree -->
      <div class="rr-honeypot" aria-hidden="true">
        <label for="rr-company">Bedrijf (niet invullen)</label>
        <input
          id="rr-company"
          v-model="form.honeypot"
          type="text"
          name="_honey"
          tabindex="-1"
          autocomplete="off"
        />
      </div>

      <fieldset class="rr-fieldset">
        <legend>Wil je bijdragen aan de (juridische) validatie van RegelRecht?</legend>
        <label class="rr-choice">
          <input
            ref="firstFieldFocus"
            v-model="form.bijdragen"
            type="radio"
            name="bijdragen"
            value="Ja"
          />
          <span>Ja, ik wil bijdragen aan de validatie van RegelRecht</span>
        </label>
        <label class="rr-choice">
          <input
            v-model="form.bijdragen"
            type="radio"
            name="bijdragen"
            value="Nee"
          />
          <span>Nee, ik wil niet bijdragen</span>
        </label>
      </fieldset>

      <div class="rr-field">
        <label class="rr-choice">
          <input v-model="form.opDeHoogte" type="checkbox" />
          <span>Ik wil updates ontvangen over de ontwikkelingen van RegelRecht</span>
        </label>
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-email">
          E-mailadres <span class="rr-required" aria-hidden="true">*</span>
          <span class="rr-visually-hidden">(verplicht)</span>
        </label>
        <input
          id="rr-email"
          ref="emailInput"
          v-model="form.email"
          type="email"
          name="email"
          autocomplete="email"
          required
          aria-required="true"
          :aria-invalid="errors.email ? 'true' : undefined"
          :aria-describedby="errors.email ? 'rr-email-error' : undefined"
        />
        <p v-if="errors.email" id="rr-email-error" class="rr-error-text">
          {{ errors.email }}
        </p>
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-naam">
          Volledige naam <span class="rr-required" aria-hidden="true">*</span>
          <span class="rr-visually-hidden">(verplicht)</span>
        </label>
        <input
          id="rr-naam"
          ref="naamInput"
          v-model="form.naam"
          type="text"
          name="naam"
          autocomplete="name"
          required
          aria-required="true"
          :aria-invalid="errors.naam ? 'true' : undefined"
          :aria-describedby="errors.naam ? 'rr-naam-error' : undefined"
        />
        <p v-if="errors.naam" id="rr-naam-error" class="rr-error-text">
          {{ errors.naam }}
        </p>
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-org">Organisatie (optioneel)</label>
        <input
          id="rr-org"
          v-model="form.organisatie"
          type="text"
          name="organisatie"
          autocomplete="organization"
          placeholder="Bijv. Ministerie van BZK, Gemeente Amsterdam"
        />
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-functie">Functie (optioneel)</label>
        <input
          id="rr-functie"
          v-model="form.functie"
          type="text"
          name="functie"
          autocomplete="organization-title"
          placeholder="Bijv. jurist, beleidsmedewerker, wetgevingsjurist"
        />
      </div>

      <p class="rr-form-status" role="status" aria-live="polite">
        <span v-if="status === 'submitting'">Bezig met versturen…</span>
      </p>

      <button
        type="submit"
        class="rr-btn rr-btn--primary"
        :disabled="status === 'submitting'"
      >
        {{ status === 'submitting' ? 'Bezig met versturen…' : 'Meld me aan' }}
      </button>
    </form>

    <div
      v-else-if="status === 'success'"
      class="rr-status-box rr-status-box--ok"
      role="status"
      aria-live="polite"
    >
      <h3 ref="successHeading" tabindex="-1">Bedankt voor je aanmelding!</h3>
      <p>
        We hebben je gegevens verstuurd. Je ontvangt bevestiging per e-mail
        zodra je aanmelding is verwerkt. Klopt er iets niet? Mail dan naar
        <a href="mailto:regelrecht@minbzk.nl">regelrecht@minbzk.nl</a>.
      </p>
      <button type="button" class="rr-btn rr-btn--ghost" @click="reset">
        Nog iemand aanmelden
      </button>
    </div>

    <div
      v-else
      class="rr-status-box rr-status-box--err"
      role="alert"
      aria-live="assertive"
    >
      <h3 ref="errorHeading" tabindex="-1">Er ging iets mis</h3>
      <p>
        Het versturen is niet gelukt. Probeer het opnieuw of stuur een e-mail
        naar <a href="mailto:regelrecht@minbzk.nl">regelrecht@minbzk.nl</a>.
      </p>
      <button type="button" class="rr-btn rr-btn--ghost" @click="reset">
        Opnieuw proberen
      </button>
    </div>
  </div>
</template>
