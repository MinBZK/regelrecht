<script setup lang="ts">
import { data as rfcs } from '../../../rfcs/index.data'
</script>

<template>
  <table>
    <thead>
      <tr>
        <th>RFC</th>
        <th>Title</th>
        <th>Status</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="rfc in rfcs" :key="rfc.num">
        <td><a :href="rfc.link">{{ rfc.id }}</a></td>
        <td>{{ rfc.title }}</td>
        <td>{{ rfc.status }}</td>
      </tr>
    </tbody>
  </table>
</template>
