<script setup>
import { reactive } from 'vue';
import { useGuide } from './composables/useGuide.js';
import MainPane from './components/MainPane.vue';
import SidebarPane from './components/SidebarPane.vue';
import SourcePane from './components/SourcePane.vue';

// `reactive` unwraps the composable's refs, so panes read `g.crateView`, not `g.crateView.value`.
const g = reactive(useGuide());
</script>

<template>
  <nldd-app-view>
    <nldd-navigation-split-view
      primary-sidebar-accessible-label="Crates and modules"
      inspector-accessible-label="Source code"
    >
      <nldd-split-view-pane slot="primary-sidebar" has-content>
        <SidebarPane :g="g" />
      </nldd-split-view-pane>
      <!-- Widened, the source takes the main pane and the inspector goes away. -->
      <nldd-split-view-pane slot="main" has-content>
        <SourcePane v-if="g.route.wide" :g="g" />
        <MainPane v-else :g="g" />
      </nldd-split-view-pane>
      <nldd-split-view-pane v-if="!g.route.wide" slot="inspector" has-content>
        <SourcePane :g="g" />
      </nldd-split-view-pane>
    </nldd-navigation-split-view>
  </nldd-app-view>
</template>
