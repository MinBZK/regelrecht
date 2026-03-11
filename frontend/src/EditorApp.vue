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

    <!-- Content area -->
    <rr-simple-section>
      <p>Editor wordt geladen...</p>
    </rr-simple-section>
  </rr-page>
</template>

<script setup>
</script>
