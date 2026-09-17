<script setup>
import { onMounted } from 'vue';
import { refreshSession } from '../session.js';

onMounted(refreshSession);
</script>

<template>
  <nldd-app-view>
    <router-view />
  </nldd-app-view>
</template>
