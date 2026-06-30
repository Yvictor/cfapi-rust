#pragma once

#include "cfapi.h"
#include <cstdint>
#include <memory>

class RustMessageEventHandler : public cfapi::MessageEventHandler
{
public:
    explicit RustMessageEventHandler(std::uintptr_t handler_id);
    void onMessageEvent(const cfapi::MessageEvent &event) override;

private:
    std::uintptr_t handler_id;
};

extern "C" void cfapi_rust_on_message_event(std::uintptr_t handler_id, const cfapi::MessageEvent &event);

class APIFactoryWrap
{
    // protected:
public:
    cfapi::APIFactory *ptr;
    cfapi::Session *session;
    cfapi::UserInfo *primaryUser;
    std::unique_ptr<RustMessageEventHandler> rustMessageHandler;

    APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                   bool debug, const std::string &logFileName, std::string usage,
                   const std::string &username, const std::string &password,
                   const cfapi::UserEventHandler &userHandler,
                   const cfapi::SessionEventHandler &sessionHandler);
    ~APIFactoryWrap();
    void setSessionConfigInt(cfapi::SessionConfig::Parameters param, long value);
    void setSessionConfigBool(cfapi::SessionConfig::Parameters param, bool value);
    void setGlobalConnectionConfig(bool backup, bool compression,
                                   bool conflation_indicator, long conflation_interval,
                                   long read_timeout, long connection_timeout,
                                   long connection_retry_limit, long queue_size,
                                   long blocking_connection_time_limit, long conflation_type,
                                   long jit_conflation_threshold_percent);
    void setConnectionConfig(std::string &host_info, bool backup, bool compression,
                             bool conflation_indicator, long conflation_interval,
                             long read_timeout, long connection_timeout,
                             long connection_retry_limit, long queue_size,
                             long blocking_connection_time_limit, long conflation_type,
                             long jit_conflation_threshold_percent);
    bool startSession();
    int64_t sendRequest(const std::string &src_id, const std::string &symbol, cfapi::Commands command);
    int64_t sendCommand(cfapi::Commands command);
    // void registerMessageEventHandler(cfapi::MessageEventHandler *messageHandler);
    void registerMessageEventHandler(const cfapi::MessageEventHandler &messageHandler);
    void registerRustMessageEventHandler(std::uintptr_t handler_id);
    void registerStatisticsEventHandler(const cfapi::StatisticsEventHandler &statsEH, int interval);
    // const cfapi::Session& getSession();
    // cfapi::Session *getSession();
};

void *GetEventReader(const cfapi::MessageEvent &event);
// const cfapi::DateTime GetDatetime(cfapi::MessageReader &reader);
void *GetDate(const cfapi::MessageReader &reader);
void *GetTime(const cfapi::MessageReader &reader);
