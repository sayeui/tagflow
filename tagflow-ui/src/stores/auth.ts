import { defineStore } from 'pinia'

export const useAuthStore = defineStore('auth', {
  state: () => ({
    token: localStorage.getItem('auth_token') || null,
    username: localStorage.getItem('username') || null,
  }),

  getters: {
    isLoggedIn: (state) => !!state.token,
  },

  actions: {
    setToken(token: string, username: string) {
      this.token = token
      this.username = username
      localStorage.setItem('auth_token', token)
      localStorage.setItem('username', username)
    },

    logout() {
      this.token = null
      this.username = null
      localStorage.removeItem('auth_token')
      localStorage.removeItem('username')
    },
  },
})
