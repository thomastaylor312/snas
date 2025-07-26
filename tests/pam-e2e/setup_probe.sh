#!/usr/bin/env bash
set -euo pipefail

cat > /tmp/pam_probe.c <<'C'
#include <security/pam_appl.h>
#include <security/pam_modules.h>
#include <stdio.h>

static pam_handle_t *global_handle = NULL;

PAM_EXTERN int pam_sm_authenticate(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    global_handle = pamh;
    return PAM_SUCCESS;
}

PAM_EXTERN int pam_sm_setcred(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    if (global_handle != pamh) {
        fprintf(stderr, "handle mismatch\n");
        return PAM_SYSTEM_ERR;
    }
    return PAM_SUCCESS;
}

PAM_EXTERN int pam_sm_acct_mgmt(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    if (global_handle != pamh) {
        fprintf(stderr, "handle mismatch acct\n");
        return PAM_SYSTEM_ERR;
    }
    return PAM_USER_UNKNOWN;
}

PAM_EXTERN int pam_sm_open_session(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    if (global_handle != pamh) {
        fprintf(stderr, "handle mismatch open\n");
        return PAM_SYSTEM_ERR;
    }
    return PAM_SUCCESS;
}

PAM_EXTERN int pam_sm_close_session(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    if (global_handle != pamh) {
        fprintf(stderr, "handle mismatch close\n");
        return PAM_SYSTEM_ERR;
    }
    return PAM_SUCCESS;
}

PAM_EXTERN int pam_sm_chauthtok(pam_handle_t *pamh, int flags, int argc, const char **argv) {
    if (global_handle != pamh) {
        fprintf(stderr, "handle mismatch chauthtok\n");
        return PAM_SYSTEM_ERR;
    }
    return PAM_SUCCESS;
}

#ifdef PAM_MODULE_ENTRY
PAM_MODULE_ENTRY("pam_probe");
#endif
C

cat > /tmp/probe <<'CONF'
#%PAM-1.0
auth       required   pam_probe.so
account    required   pam_probe.so
password   required   pam_probe.so
session    required   pam_probe.so
CONF

cc -shared -fPIC -o /lib/security/pam_probe.so /tmp/pam_probe.c
cp /tmp/probe /etc/pam.d/probe
