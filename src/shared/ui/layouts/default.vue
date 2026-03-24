<template>
    <div class="flex min-h-screen flex-col">
        <header class="shrink-0 p-4 pb-0">
            <CAppHeader />
        </header>
        <main class="relative flex grow flex-col">
            <slot />
        </main>
    </div>
</template>

<script setup lang="ts">
    import CAppHeader from '~/shared/ui/app-header/app-header.vue';
</script>
