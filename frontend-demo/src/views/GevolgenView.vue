<script setup>
import { computed } from 'vue';
import { formatValue, humanize } from '../data/format.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';

// Gevolgen: wat er buiten de overheid gebeurt door haar besluiten. Een bank
// hoort niet in Mijn overheid; hier staat per partij die geen overheid is wat
// haar cel over de persona vastlegde (`consequences` in demo-config.yaml).
// Welke partij het is, zegt de configuratie; deze pagina noemt er geen.

const demo = useDemo();
const { t } = useI18n();
const { corpus, state, persona, profile, activeDelegation, dataVersion } = demo;

const EURO = { type: 'amount' };

/** Wie het betreft: de persona, of namens wie er gehandeld wordt. */
const subjectName = computed(() => activeDelegation.value?.subjectName ?? persona.value?.name ?? profile.value?.name ?? '');

/** Een rekening zoals de pagina haar toont: saldo en elke overboeking of afschrijving. */
function accountView(a) {
  return {
    ...a,
    balanceText: formatValue(a.balance, EURO),
    transactions: a.transactions.map((tx) => ({
      id: tx.id,
      text: tx.credited
        ? t('gevolgen.account.credited')
        : tx.debited
          ? t('gevolgen.account.debited')
          : t('gevolgen.account.refused', { reason: tx.reason ?? '' }),
      supporting: [formatValue(tx.date), tx.payer ? t(tx.debited ? 'gevolgen.account.to' : 'gevolgen.account.from', { payer: humanize(tx.payer) }) : null].filter(Boolean).join(' · '),
      // Een afschrijving staat er negatief: het saldo gaat erdoor omlaag.
      amount: formatValue(tx.credited || (tx.debited ? -tx.debited : tx.amount), EURO),
    })),
  };
}

const parties = computed(() => {
  void dataVersion.value;
  void state.grams;
  return demo.consequencesOf().map((p) => ({
    ...p,
    name: p.service ? serviceInfo(corpus.value, p.service).name : humanize(p.cell),
    account: p.view === 'account' && p.data ? accountView(p.data) : null,
  }));
});
</script>

<template>
  <nldd-page>
    <nldd-simple-section width="1440px">
      <nldd-title slot="header" size="2">
        <h1>{{ t('gevolgen.title') }}</h1>
        <span slot="supporting-text">{{ t('gevolgen.intro', { name: subjectName }) }}</span>
      </nldd-title>
      <nldd-container gap="24">
        <nldd-container v-if="!parties.length" padding-inline="12">
          <nldd-text size="sm" color="secondary">{{ t('gevolgen.none') }}</nldd-text>
        </nldd-container>

        <nldd-container v-for="p in parties" :key="p.cell" gap="8">
          <nldd-title size="4">
            <h2>{{ p.name }}</h2>
            <span v-if="p.account" slot="supporting-text">{{ t('gevolgen.account.supporting', { number: p.account.number }) }}</span>
          </nldd-title>

          <!-- Een rekening: saldo en overboekingen, uit de kroniek van de cel. -->
          <template v-if="p.account">
            <nldd-banner v-if="p.account.blocked" variant="warning" :text="t('gevolgen.account.blocked')"></nldd-banner>
            <nldd-list appearance="box-tinted" :accessible-label="t('gevolgen.account.label', { party: p.name })">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" :text="t('gevolgen.account.balance')" :supporting-text="t('gevolgen.account.opening', { amount: formatValue(p.account.opening, EURO) })"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="p.account.balanceText"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-for="tx in p.account.transactions" :key="tx.id" size="sm">
                <nldd-text-cell size="sm" :text="tx.text" :supporting-text="tx.supporting"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="tx.amount"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-if="!p.account.transactions.length" size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('gevolgen.account.empty')"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
          <nldd-container v-else padding-inline="12">
            <nldd-text size="sm" color="secondary">{{ t('gevolgen.nothing', { name: subjectName }) }}</nldd-text>
          </nldd-container>
        </nldd-container>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
