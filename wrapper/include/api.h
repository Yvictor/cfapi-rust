#pragma once

#include "cfapi.h"

class APIFactoryWrap
{
    // protected:
public:
    cfapi::APIFactory *ptr;
    cfapi::Session *session;
    cfapi::UserInfo *primaryUser;

    APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                   bool debug, const std::string &logFileName, std::string usage,
                   const std::string &username, const std::string &password,
                   const cfapi::UserEventHandler &userHandler,
                   const cfapi::SessionEventHandler &sessionHandler);
    ~APIFactoryWrap();
    void setSessionConfig(cfapi::SessionConfig::Parameters param, long value);
    void setConnectionConfig(cfapi::HostConfig::Parameters param, long value);
    void setHostConfig(std::string &host_info, bool backup, bool compression);
    bool startSession();
    std::int64_t sendRequest(const std::string &src_id, const std::string &symbol, cfapi::Commands command);
    // void registerMessageEventHandler(cfapi::MessageEventHandler *messageHandler);
    void registerMessageEventHandler(const cfapi::MessageEventHandler &messageHandler);
    // const cfapi::Session& getSession();
    // cfapi::Session *getSession();
};

void *GetEventReader(const cfapi::MessageEvent &event);
// const cfapi::DateTime GetDatetime(cfapi::MessageReader &reader);
void *GetDate(const cfapi::MessageReader &reader);
void *GetTime(const cfapi::MessageReader &reader);