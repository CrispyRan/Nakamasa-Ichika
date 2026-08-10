<template>
  <st-welcome />
  <st-shortcuts />
  <st-count />
  <st-login-chart />

  <div class="block lg:flex">
    <st-announced />
    <st-saiadmin />
  </div>
</template>

<script setup>
import StCount from './components/st-count.vue'
import StWelcome from './components/st-welcome.vue'
import StLoginChart from './components/st-loginChart.vue'
import StSaiadmin from './components/st-saiadmin.vue'
import StAnnounced from './components/st-announced.vue'
import StShortcuts from './components/st-shortcuts.vue'
</script>
