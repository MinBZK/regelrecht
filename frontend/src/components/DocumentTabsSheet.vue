<script setup>
import { computed, ref } from 'vue';
import { useAppChrome } from '../composables/useAppChrome.js';

// Mobiele vervanger van de document-tab-bar. Op mobiel staat in de editor-
// toolbar een knop (naast het traject-menu) die het actieve tabblad toont en
// een sheet opent met alle open tabbladen. Zo vervalt op mobiel de aparte
// tab-bar-rij — één toolbar-laag minder.
//
// Leest de open tabbladen + acties uit de gedeelde AppShell-chrome; de editor
// houdt die in sync (zelfde bron als de md+ document-tab-bar).
const { documentTabs, activeDocumentTab, tabActions } = useAppChrome();

const sheetEl = ref(null);
function openSheet() {
  sheetEl.value?.show();
}
function closeSheet() {
  sheetEl.value?.hide();
}

// Labels: artikel als hoofdtekst, wetgeving als ondersteunende tekst — gelijk
// aan de document-tab-bar.
function tabText(tab) {
  return `Artikel ${tab.articleNumber}`;
}
function tabSupporting(tab) {
  return tabActions.value?.displayName(tab);
}

// De knop toont het actieve tabblad (valt terug op een generieke tekst zolang
// er nog geen actief tabblad is).
const triggerText = computed(() =>
  activeDocumentTab.value ? tabText(activeDocumentTab.value) : 'Tabbladen',
);
const triggerSupporting = computed(() =>
  activeDocumentTab.value ? tabSupporting(activeDocumentTab.value) : undefined,
);

function isActive(tab) {
  const active = activeDocumentTab.value;
  return !!(
    active &&
    tabActions.value &&
    tabActions.value.key(active) === tabActions.value.key(tab)
  );
}

function selectTab(tab) {
  tabActions.value?.select(tab);
  closeSheet();
}

function closeTab(tab) {
  // De editor-closeTab wijst het actieve tabblad opnieuw toe; de lijst werkt
  // reactief bij. Sluit het laatste tabblad → AppShell verbergt deze knop
  // (en daarmee de sheet) omdat er geen tabbladen meer zijn.
  tabActions.value?.close(tab);
}

// De nldd-list dispatcht nldd-reorder met { fromIndex, toIndex }; de editor
// muteert openTabs en de v-for hieronder rendert de nieuwe volgorde.
function onReorder(e) {
  const { fromIndex, toIndex } = e.detail || {};
  if (typeof fromIndex === 'number' && typeof toIndex === 'number') {
    tabActions.value?.reorder?.(fromIndex, toIndex);
  }
}
</script>

<template>
  <nldd-button
    size="md"
    expandable
    width="full"
    horizontal-alignment="left"
    single-line
    start-icon="document"
    :text="triggerText"
    :supporting-text="triggerSupporting"
    @click="openSheet"
  ></nldd-button>

  <!-- Teleport naar body zodat de sheet buiten de clipping van de toolbar
       valt (zelfde aanpak als TrajectDocuments). -->
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="bottom">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Tabbladen"
          dismiss-text="Sluit"
          @dismiss="closeSheet"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-list variant="simple" reorderable @nldd-reorder="onReorder">
            <nldd-list-item
              v-for="tab in documentTabs"
              :key="tabActions.key(tab)"
              size="md"
              button
              :selected="isActive(tab) || undefined"
              @click="selectTab(tab)"
            >
              <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
              <nldd-drag-handle-cell slot="start" size="sm" reorderable-only></nldd-drag-handle-cell>
              <nldd-spacer-cell slot="start" size="8" reorderable-only></nldd-spacer-cell>
              <nldd-text-cell
                :text="tabText(tab)"
                :supporting-text="tabSupporting(tab)"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-icon-button
                  size="sm"
                  icon="dismiss"
                  text="Sluit tabblad"
                  @click.stop="closeTab(tab)"
                ></nldd-icon-button>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
