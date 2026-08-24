import { mount } from 'svelte'
import '@fontsource-variable/archivo/wdth.css'
import '@fontsource-variable/jetbrains-mono/index.css'
import './app.css'
import App from './App.svelte'

const target = document.getElementById('app')
mount(App, { target })
