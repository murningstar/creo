<template>
    <div class="space-y-2">
        <!-- Add button disguised as list item -->
        <button
            class="bg-muted flex w-full items-center justify-center gap-2 rounded-md border border-dashed border-neutral-300 px-3 py-2 transition-colors hover:border-neutral-400 dark:border-neutral-600 dark:hover:border-neutral-500"
            @click="emit('add')"
        >
            <u-icon name="i-lucide-plus" class="text-dimmed size-4" />
            <span class="text-dimmed text-xs">{{ label }}</span>
        </button>

        <!-- List items via slot -->
        <slot />
    </div>
</template>

<script setup lang="ts">
    withDefaults(
        defineProps<{
            label?: string;
        }>(),
        {
            label: 'New',
        }
    );

    const emit = defineEmits<{
        add: [];
    }>();
</script>
