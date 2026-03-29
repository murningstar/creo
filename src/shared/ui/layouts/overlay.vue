<template>
    <div class="overlay-root">
        <slot />
    </div>
</template>

<style scoped>
    .overlay-root {
        width: 100vw;
        height: 100vh;
        overflow: hidden;
        background: transparent;
    }
</style>
