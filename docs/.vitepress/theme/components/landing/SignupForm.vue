<script setup lang="ts">
import { reactive, ref, computed, nextTick } from 'vue'
import { useRoute } from 'vitepress'
import { content, langFromPath } from './content'

const WEBHOOK =
  'https://digilab.overheid.nl/chat/hooks/khcsah5zg3gy8notbfy5baoxwh'
const EMAIL = 'regelrecht@minbzk.nl'

const route = useRoute()
const lang = computed(() => langFromPath(route.path))
const s = computed(() => content[lang.value].signup)

// Split a string on the support email so it can be rendered as a mailto link.
function parts(text: string) {
  const i = text.indexOf(EMAIL)
  if (i === -1) return { before: text, after: '' }
  return { before: text.slice(0, i), after: text.slice(i + EMAIL.length) }
}
const errorParts = computed(() => parts(s.value.errorBody))
const successParts = computed(() => parts(s.value.successBody))

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
    errors.email = s.value.errEmailEmpty
  } else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim())) {
    errors.email = s.value.errEmailInvalid
  }
  if (!form.naam.trim()) {
    errors.naam = s.value.errName
  }
  return !errors.email && !errors.naam
}

async function onSubmit() {
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
      <div class="rr-honeypot" aria-hidden="true">
        <label for="rr-company">{{ s.companyHoneypot }}</label>
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
        <legend>{{ s.legend }}</legend>
        <label class="rr-choice">
          <input
            ref="firstFieldFocus"
            v-model="form.bijdragen"
            type="radio"
            name="bijdragen"
            value="Ja"
          />
          <span>{{ s.radioYes }}</span>
        </label>
        <label class="rr-choice">
          <input
            v-model="form.bijdragen"
            type="radio"
            name="bijdragen"
            value="Nee"
          />
          <span>{{ s.radioNo }}</span>
        </label>
      </fieldset>

      <div class="rr-field">
        <label class="rr-choice">
          <input v-model="form.opDeHoogte" type="checkbox" />
          <span>{{ s.updates }}</span>
        </label>
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-email">
          {{ s.emailLabel }}
          <span class="rr-required" aria-hidden="true">{{ s.required }}</span>
          <span class="rr-visually-hidden">{{ s.requiredSr }}</span>
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
          {{ s.nameLabel }}
          <span class="rr-required" aria-hidden="true">{{ s.required }}</span>
          <span class="rr-visually-hidden">{{ s.requiredSr }}</span>
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
        <label class="rr-label" for="rr-org">{{ s.orgLabel }}</label>
        <input
          id="rr-org"
          v-model="form.organisatie"
          type="text"
          name="organisatie"
          autocomplete="organization"
          :placeholder="s.orgPlaceholder"
        />
      </div>

      <div class="rr-field">
        <label class="rr-label" for="rr-functie">{{ s.roleLabel }}</label>
        <input
          id="rr-functie"
          v-model="form.functie"
          type="text"
          name="functie"
          autocomplete="organization-title"
          :placeholder="s.rolePlaceholder"
        />
      </div>

      <p class="rr-form-status" role="status" aria-live="polite">
        <span v-if="status === 'submitting'">{{ s.submitting }}</span>
      </p>

      <button
        type="submit"
        class="rr-btn rr-btn--primary"
        :disabled="status === 'submitting'"
      >
        {{ status === 'submitting' ? s.submitting : s.submit }}
      </button>
    </form>

    <div
      v-else-if="status === 'success'"
      class="rr-status-box rr-status-box--ok"
      role="status"
      aria-live="polite"
    >
      <h3 ref="successHeading" tabindex="-1">{{ s.successTitle }}</h3>
      <p>
        {{ successParts.before
        }}<a :href="`mailto:${EMAIL}`">{{ EMAIL }}</a
        >{{ successParts.after }}
      </p>
      <button type="button" class="rr-btn rr-btn--ghost" @click="reset">
        {{ s.successReset }}
      </button>
    </div>

    <div
      v-else
      class="rr-status-box rr-status-box--err"
      role="alert"
      aria-live="assertive"
    >
      <h3 ref="errorHeading" tabindex="-1">{{ s.errorTitle }}</h3>
      <p>
        {{ errorParts.before
        }}<a :href="`mailto:${EMAIL}`">{{ EMAIL }}</a
        >{{ errorParts.after }}
      </p>
      <button type="button" class="rr-btn rr-btn--ghost" @click="reset">
        {{ s.errorReset }}
      </button>
    </div>
  </div>
</template>
