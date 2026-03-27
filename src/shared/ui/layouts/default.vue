<template>
    <div class="flex h-screen flex-col overflow-y-auto" style="scrollbar-gutter: stable">
        <header class="shrink-0 p-4 pb-0">
            <div class="flex items-center gap-3">
                <slot name="voice-status" />

                <!-- Navigation -->
                <div class="flex flex-1 justify-center">
                    <u-tabs
                        :model-value="currentTab"
                        :items="tabs"
                        :content="false"
                        :ui="{ root: 'w-fit' }"
                        variant="link"
                        size="xl"
                        @update:model-value="navigate"
                    />
                </div>
            </div>
        </header>
        <main class="relative flex grow flex-col">
            <slot />
        </main>
    </div>
</template>

<script setup lang="ts">
    const props = withDefaults(
        defineProps<{
            appLabel?: string;
        }>(),
        { appLabel: 'Home' }
    );

    const route = useRoute();
    const router = useRouter();

    const tabs = computed(() => [
        { label: props.appLabel, value: '/', icon: 'i-lucide-layout-dashboard' },
        { label: 'Settings', value: '/settings', icon: 'i-lucide-settings' },
    ]);

    const currentTab = computed(() => route.path);

    function navigate(value: string | number) {
        router.push(String(value));
    }
</script>
