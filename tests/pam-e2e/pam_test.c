#define _GNU_SOURCE
#include <security/pam_appl.h>
#include <security/pam_modules.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct Passwords {
  const char *password;
  const char *new_password;
  const char *verify_password;
} Passwords;

static int conv_func(int num_msg, const struct pam_message **msg,
                     struct pam_response **resp, void *appdata_ptr) {
  if (num_msg <= 0)
    return PAM_CONV_ERR;
  *resp = calloc(num_msg, sizeof(struct pam_response));
  if (!*resp)
    return PAM_CONV_ERR;

  Passwords *pw = (Passwords *)appdata_ptr;
  for (int i = 0; i < num_msg; i++) {
    const struct pam_message *m = msg[i];
    switch (m->msg_style) {
    case PAM_PROMPT_ECHO_OFF: {
      // Determine which prompt this is by its text
      const char *ans = NULL;
      if (strstr(m->msg, "Current Password")) {
        ans = pw->password;
      } else if (strstr(m->msg, "New Password")) {
        ans = pw->new_password ? pw->new_password : pw->password;
      } else if (strstr(m->msg, "Verify Password")) {
        ans = pw->verify_password ? pw->verify_password : pw->new_password;
      } else {
        // Fallback: use main password for generic prompt
        ans = pw->password;
      }
      (*resp)[i].resp = strdup(ans ? ans : "");
      (*resp)[i].resp_retcode = 0;
      break;
    }
    case PAM_PROMPT_ECHO_ON: {
      (*resp)[i].resp = strdup("");
      (*resp)[i].resp_retcode = 0;
      break;
    }
    case PAM_ERROR_MSG:
    case PAM_TEXT_INFO:
      // ignore informational messages
      (*resp)[i].resp = NULL;
      (*resp)[i].resp_retcode = 0;
      break;
    default:
      free(*resp);
      *resp = NULL;
      return PAM_CONV_ERR;
    }
  }
  return PAM_SUCCESS;
}

static int do_auth(const char *service, const char *user, Passwords *pw) {
  struct pam_conv conv = {conv_func, pw};
  pam_handle_t *pamh = NULL;
  int ret = pam_start(service, user, &conv, &pamh);
  if (ret != PAM_SUCCESS) {
    fprintf(stderr, "pam_start failed: %s\n", pam_strerror(pamh, ret));
    return ret;
  }

  ret = pam_authenticate(pamh, 0);
  if (ret != PAM_SUCCESS && ret != PAM_NEW_AUTHTOK_REQD) {
    fprintf(stderr, "pam_authenticate: %s\n", pam_strerror(pamh, ret));
    pam_end(pamh, ret);
    return ret;
  }

  int acct = pam_acct_mgmt(pamh, 0);
  if (acct == PAM_NEW_AUTHTOK_REQD || ret == PAM_NEW_AUTHTOK_REQD) {
    int cr = pam_chauthtok(pamh, 0);
    if (cr != PAM_SUCCESS) {
      fprintf(stderr, "pam_chauthtok: %s\n", pam_strerror(pamh, cr));
      pam_end(pamh, cr);
      return cr;
    }
  } else if (acct != PAM_SUCCESS) {
    fprintf(stderr, "pam_acct_mgmt: %s\n", pam_strerror(pamh, acct));
    pam_end(pamh, acct);
    return acct;
  }

  int sc = pam_setcred(pamh, PAM_ESTABLISH_CRED);
  if (sc != PAM_SUCCESS) {
    fprintf(stderr, "pam_setcred: %s\n", pam_strerror(pamh, sc));
    pam_end(pamh, sc);
    return sc;
  }

  pam_end(pamh, PAM_SUCCESS);
  return PAM_SUCCESS;
}

int main(int argc, char **argv) {
  if (argc < 4) {
    fprintf(stderr,
            "Usage: %s <service> <username> <password> [new_password] [verify_password]\n",
            argv[0]);
    return 2;
  }
  const char *service = argv[1];
  const char *user = argv[2];
  Passwords pw = {
      .password = argv[3],
      .new_password = (argc >= 5 ? argv[4] : NULL),
      .verify_password = (argc >= 6 ? argv[5] : NULL),
  };

  int ret = do_auth(service, user, &pw);
  if (ret != PAM_SUCCESS) {
    fprintf(stderr, "Authentication flow failed with code %d\n", ret);
    return 1;
  }
  printf("PAM authentication OK\n");
  return 0;
}

