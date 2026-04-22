<script setup>
import { useRouter } from 'vue-router';

defineProps({ lawId: { type: String, required: true } });
const router = useRouter();
</script>

<template>
  <ndd-page>
    <ndd-container padding="24">
      <ndd-back-button
        text="Terug"
        @click="router.push({ name: 'law-detail', params: { lawId } })"
      ></ndd-back-button>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-title size="2"><h1>Populatie-simulatie</h1></ndd-title>
      <p>Nog niet beschikbaar — zie Fase 6 van RFC-016.</p>
    </ndd-container>
  </ndd-page>
</template>
