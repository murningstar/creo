<template>
    <div class="flex items-center gap-3">
        <h1 class="text-lg font-semibold">Creo</h1>

        <div class="flex flex-1 justify-center">
            <u-tabs
                :model-value="currentTab"
                :items="tabs"
                :content="false"
                variant="link"
                size="xl"
                @update:model-value="navigate"
            />
        </div>
    </div>
</template>

<script setup lang="ts">
    const route = useRoute();
    const router = useRouter();

    const tabs = [
        { label: 'Creo', value: '/', icon: 'i-lucide-layout-dashboard' },
        { label: 'Settings', value: '/settings', icon: 'i-lucide-settings' },
    ];

    const currentTab = computed(() => route.path);

    function navigate(value: string | number) {
        router.push(String(value));
    }
</script>
