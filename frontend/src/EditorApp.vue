<script setup>
import { ref } from 'vue';
import { useLaw } from './composables/useLaw.js';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';

const { articles, lawName, selectedArticle, selectedArticleNumber, loading, error } = useLaw();

const deploymentName = window.__ENV?.DEPLOYMENT_NAME || '';
const isPreview = deploymentName && deploymentName !== 'regelrecht';

const activeAction = ref(null);
const rightPaneView = ref('machine');

function selectArticle(number) {
  activeAction.value = null;
  selectedArticleNumber.value = String(number);
}
</script>

<template>
  <rr-page header-sticky>
    <!-- Header: Main Toolbar -->
    <rr-toolbar slot="header" size="md">
      <rr-toolbar-start-area>
        <rr-toolbar-item>
          <rr-tab-bar size="md">
            <rr-tab-bar-item href="index.html">Bibliotheek</rr-tab-bar-item>
            <rr-tab-bar-item selected>Editor</rr-tab-bar-item>
          </rr-tab-bar>
        </rr-toolbar-item>
      </rr-toolbar-start-area>
      <rr-toolbar-center-area>
        <rr-toolbar-item>
          <rr-search-field size="md" placeholder="Zoeken"></rr-search-field>
        </rr-toolbar-item>
      </rr-toolbar-center-area>
      <rr-toolbar-end-area>
        <rr-toolbar-item>
          <rr-icon-button variant="neutral-tinted" size="m" title="Notificaties">
            <img slot="__icon" src="/assets/icons/bell.svg" alt="Notificaties" width="24" height="24">
          </rr-icon-button>
        </rr-toolbar-item>
        <rr-toolbar-item v-if="isPreview">
          <span class="env-badge">{{ deploymentName }}</span>
        </rr-toolbar-item>
        <rr-toolbar-item>
          <rr-button-bar size="md">
            <rr-button variant="neutral-tinted" size="md" is-picker>RR Project</rr-button>
            <rr-icon-button variant="neutral-tinted" size="m" has-menu title="Account">
              <img slot="__icon" src="/assets/icons/person.svg" alt="Account" width="24" height="24">
            </rr-icon-button>
          </rr-button-bar>
        </rr-toolbar-item>
      </rr-toolbar-end-area>
    </rr-toolbar>

    <!-- Error state -->
    <div v-if="error" style="padding: 32px; color: #c00; text-align: center;">
      Kon de wet niet laden: {{ error.message }}
    </div>

    <!-- Document Tab Bar -->
    <rr-document-tab-bar v-if="!loading && !error">
      <rr-document-tab-bar-item
        v-for="article in articles"
        :key="article.number"
        :subtitle="lawName"
        :selected="String(article.number) === String(selectedArticleNumber) || undefined"
        has-dismiss-button
        @click="selectArticle(article.number)"
      >
        Artikel {{ article.number }}
      </rr-document-tab-bar-item>
    </rr-document-tab-bar>

    <!-- Main Editor: Side-by-side split view -->
    <rr-side-by-side-split-view>

      <!-- Left Pane: Text -->
      <div slot="pane-1" style="background: #F4F6F9;">
        <rr-page header-sticky>
          <rr-toolbar slot="header" size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <rr-button variant="neutral-tinted" size="md">
                  Tekst
                  <img src="/assets/icons/chevron-down-small.svg" alt="" width="16" height="16">
                </rr-button>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
            <rr-toolbar-end-area>
              <rr-toolbar-item>
                <rr-segmented-control size="md" content-type="icons">
                  <rr-segmented-control-item value="bold" title="Bold">
                    <img src="/assets/icons/bold.svg" alt="Bold" width="20" height="20">
                  </rr-segmented-control-item>
                  <rr-segmented-control-item value="italic" title="Italic">
                    <img src="/assets/icons/italic.svg" alt="Italic" width="20" height="20">
                  </rr-segmented-control-item>
                </rr-segmented-control>
              </rr-toolbar-item>
              <rr-toolbar-item>
                <rr-segmented-control size="md" content-type="icons">
                  <rr-segmented-control-item value="hr" title="Horizontale lijn">
                    <img src="/assets/icons/minus.svg" alt="Lijn" width="20" height="20">
                  </rr-segmented-control-item>
                  <rr-segmented-control-item value="ul" title="Bullet list">
                    <img src="/assets/icons/bullet-list.svg" alt="Bullet list" width="20" height="20">
                  </rr-segmented-control-item>
                  <rr-segmented-control-item value="ol" title="Numbered list">
                    <img src="/assets/icons/numbered-list.svg" alt="Numbered list" width="20" height="20">
                  </rr-segmented-control-item>
                </rr-segmented-control>
              </rr-toolbar-item>
            </rr-toolbar-end-area>
          </rr-toolbar>

          <rr-simple-section>
            <ArticleText :article="selectedArticle" />
          </rr-simple-section>
        </rr-page>
      </div>

      <!-- Right Pane: Machine / YAML -->
      <div slot="pane-2">
        <rr-page header-sticky>
          <rr-toolbar slot="header" size="md">
            <rr-toolbar-start-area>
              <rr-toolbar-item>
                <rr-segmented-control size="md" value="machine" @change="rightPaneView = $event.detail.value">
                  <rr-segmented-control-item value="machine">Machine</rr-segmented-control-item>
                  <rr-segmented-control-item value="yaml">YAML</rr-segmented-control-item>
                </rr-segmented-control>
              </rr-toolbar-item>
            </rr-toolbar-start-area>
          </rr-toolbar>

          <rr-simple-section v-show="rightPaneView === 'machine'">
            <MachineReadable :article="selectedArticle" @open-action="activeAction = $event" />
          </rr-simple-section>
          <rr-simple-section v-show="rightPaneView === 'yaml'">
            <YamlView :article="selectedArticle" />
          </rr-simple-section>
        </rr-page>
      </div>

    </rr-side-by-side-split-view>
  </rr-page>

  <ActionSheet :action="activeAction" :article="selectedArticle" @close="activeAction = null" />
</template>

<style scoped>
.env-badge {
  display: inline-block;
  padding: 2px 8px;
  font-size: var(--font-size-xs, 12px);
  font-weight: var(--font-weight-semibold, 700);
  color: #854d0e;
  background: #fef9c3;
  border: 1px solid #fde68a;
  border-radius: var(--border-radius-sm, 4px);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
</style>
